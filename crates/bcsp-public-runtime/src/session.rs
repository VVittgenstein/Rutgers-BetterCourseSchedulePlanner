use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use bcsp_application::SessionNonce;
use serde::Serialize;
use thiserror::Error;

const DOCUMENT_SESSION_TTL: Duration = Duration::from_secs(2 * 60 * 60);
/// Registry capacity. The H4 global WebSocket cap in [`crate::capacity`] is
/// deliberately far below this so leased (unevictable) sessions can never
/// crowd out issuance; the relationship is pinned there.
pub(crate) const MAX_DOCUMENT_SESSIONS: usize = 4_096;
/// Per-session WebSocket connection cap enforced inside the reserve_ws
/// lease (alert-delivery design v3.1, section 2b). The public client holds
/// one watch socket per document; the headroom covers reconnect overlap.
pub(crate) const MAX_WS_CONNECTIONS_PER_SESSION: u32 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum PublicLocale {
    #[serde(rename = "en-US")]
    EnUs,
    #[serde(rename = "zh-CN")]
    ZhCn,
}

impl PublicLocale {
    pub const fn html_lang(self) -> &'static str {
        match self {
            Self::EnUs => "en-US",
            Self::ZhCn => "zh-CN",
        }
    }
}

pub fn negotiate_locale(accept_language: Option<&str>) -> PublicLocale {
    let Some(header) = accept_language else {
        return PublicLocale::EnUs;
    };
    let mut best: Option<(u16, usize, PublicLocale)> = None;
    for (position, item) in header.split(',').enumerate() {
        let mut fields = item.trim().split(';');
        let range = fields.next().unwrap_or_default().trim();
        let mut quality = 1_000;
        let mut valid = true;
        for parameter in fields {
            let parameter = parameter.trim();
            if let Some(value) = parameter.strip_prefix("q=") {
                match parse_quality(value) {
                    Some(value) => quality = value,
                    None => valid = false,
                }
            }
        }
        if !valid || quality == 0 {
            continue;
        }
        let Some(locale) = locale_for_range(range) else {
            continue;
        };
        if best.is_none_or(|(best_quality, best_position, _)| {
            quality > best_quality || (quality == best_quality && position < best_position)
        }) {
            best = Some((quality, position, locale));
        }
    }
    best.map_or(PublicLocale::EnUs, |(_, _, locale)| locale)
}

/// Resolves one explicit locale tag (the validate request body's `locale`
/// field) with the same support boundaries as `Accept-Language` negotiation.
pub(crate) fn locale_for_tag(value: &str) -> Option<PublicLocale> {
    locale_for_range(value)
}

fn locale_for_range(value: &str) -> Option<PublicLocale> {
    let value = value.trim().to_ascii_lowercase();
    if value == "en" || value.starts_with("en-") {
        Some(PublicLocale::EnUs)
    } else if matches!(value.as_str(), "zh" | "zh-cn" | "zh-hans" | "zh-sg")
        || value.starts_with("zh-hans-")
    {
        Some(PublicLocale::ZhCn)
    } else {
        None
    }
}

fn parse_quality(value: &str) -> Option<u16> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if fraction.len() > 3 || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let fraction = format!("{fraction:0<3}").parse::<u16>().ok().unwrap_or(0);
    match whole {
        "0" => Some(fraction),
        "1" if fraction == 0 => Some(1_000),
        _ => None,
    }
}

#[derive(Clone, Copy)]
struct DocumentSession {
    locale: PublicLocale,
    last_activity: Instant,
    /// Live WebSocket connections holding a [`WsSessionLease`] on this
    /// session. A leased session is pinned: it is never TTL-pruned and never
    /// capacity-evicted while the count is non-zero, which is what keeps an
    /// idle-HTTP page with a live socket from losing its nonce (design:
    /// "WS 活动续期 nonce").
    active_ws_count: u32,
}

pub(crate) struct DocumentSessionRegistry {
    sessions: Mutex<BTreeMap<String, DocumentSession>>,
    maximum_sessions: usize,
}

impl Default for DocumentSessionRegistry {
    fn default() -> Self {
        Self {
            sessions: Mutex::new(BTreeMap::new()),
            maximum_sessions: MAX_DOCUMENT_SESSIONS,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum DocumentSessionError {
    #[error("public document-session state is unavailable")]
    Unavailable,
    /// Every registered session holds at least one live WebSocket lease, so
    /// nothing is evictable. Design status contract: capacity exhaustion is
    /// always 503 (429 stays reserved for rate limiting).
    #[error("public document-session registry is at capacity with only leased sessions")]
    CapacityExhausted,
}

/// Outcome of the atomic validate-or-renew primitive behind
/// `POST /api/v1/session/validate`.
pub(crate) enum ValidateOutcome {
    /// The supplied nonce is registered; its activity window was touched.
    Valid,
    /// The supplied nonce is gone; a replacement was issued in the same
    /// lock acquisition (sign-new-and-discard-old is atomic by construction:
    /// the old key is absent, the new key is inserted before unlock).
    Renewed(SessionNonce),
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum ReserveWsError {
    #[error("public document-session state is unavailable")]
    Unavailable,
    #[error("the session nonce is not registered")]
    UnknownSession,
    #[error("the session already holds the maximum number of WebSocket connections")]
    ConnectionLimit,
}

/// RAII lease pairing one admitted WebSocket connection with its document
/// session. Held across the whole `serve_websocket` pump; dropping it (any
/// exit path, including task abort) decrements the session's connection
/// count and touches its activity window.
pub(crate) struct WsSessionLease {
    registry: Arc<DocumentSessionRegistry>,
    nonce: String,
}

impl Drop for WsSessionLease {
    fn drop(&mut self) {
        let Ok(mut sessions) = self.registry.sessions.lock() else {
            // Poisoned lock: the registry is already reporting Unavailable
            // on every path; nothing recoverable from a Drop.
            tracing::error!(code = "PUBLIC_DOCUMENT_STATE_UNAVAILABLE");
            return;
        };
        if let Some(session) = sessions.get_mut(&self.nonce) {
            session.active_ws_count = session.active_ws_count.saturating_sub(1);
            session.last_activity = Instant::now();
        }
    }
}

impl DocumentSessionRegistry {
    pub(crate) fn issue(&self, locale: PublicLocale) -> Result<SessionNonce, DocumentSessionError> {
        self.issue_at(locale, Instant::now())
    }

    fn issue_at(
        &self,
        locale: PublicLocale,
        now: Instant,
    ) -> Result<SessionNonce, DocumentSessionError> {
        let mut sessions = self.lock()?;
        prune(&mut sessions, now);
        Self::admit_new_session(&mut sessions, self.maximum_sessions, locale, now)
    }

    /// Capacity discipline shared by issuance and renewal: at capacity the
    /// least-recently-active UNLEASED session is evicted; when every session
    /// is leased, admission fails closed with [`DocumentSessionError::CapacityExhausted`].
    fn admit_new_session(
        sessions: &mut BTreeMap<String, DocumentSession>,
        maximum_sessions: usize,
        locale: PublicLocale,
        now: Instant,
    ) -> Result<SessionNonce, DocumentSessionError> {
        if sessions.len() >= maximum_sessions {
            let oldest = sessions
                .iter()
                .filter(|(_, session)| session.active_ws_count == 0)
                .min_by(|(left_key, left), (right_key, right)| {
                    left.last_activity
                        .cmp(&right.last_activity)
                        .then_with(|| left_key.cmp(right_key))
                })
                .map(|(key, _)| key.clone());
            match oldest {
                Some(oldest) => {
                    sessions.remove(&oldest);
                }
                None => return Err(DocumentSessionError::CapacityExhausted),
            }
        }
        let nonce = SessionNonce::generate();
        sessions.insert(
            nonce.as_str().to_owned(),
            DocumentSession {
                locale,
                last_activity: now,
                active_ws_count: 0,
            },
        );
        Ok(nonce)
    }

    pub(crate) fn validate_or_renew(
        &self,
        nonce: &str,
        locale: PublicLocale,
    ) -> Result<ValidateOutcome, DocumentSessionError> {
        self.validate_or_renew_at(nonce, locale, Instant::now())
    }

    fn validate_or_renew_at(
        &self,
        nonce: &str,
        locale: PublicLocale,
        now: Instant,
    ) -> Result<ValidateOutcome, DocumentSessionError> {
        let mut sessions = self.lock()?;
        prune(&mut sessions, now);
        if let Some(session) = sessions.get_mut(nonce) {
            session.last_activity = now;
            return Ok(ValidateOutcome::Valid);
        }
        Self::admit_new_session(&mut sessions, self.maximum_sessions, locale, now)
            .map(ValidateOutcome::Renewed)
    }

    /// Atomic WebSocket admission (design: reserve_ws): validity check,
    /// per-session connection cap, count increment, and activity touch in one
    /// lock acquisition. The returned lease closes the check-then-upgrade
    /// TOCTOU window -- a leased session cannot be pruned or evicted while
    /// the upgrade completes.
    pub(crate) fn reserve_ws(
        self: &Arc<Self>,
        nonce: &str,
    ) -> Result<WsSessionLease, ReserveWsError> {
        self.reserve_ws_at(nonce, Instant::now())
    }

    fn reserve_ws_at(
        self: &Arc<Self>,
        nonce: &str,
        now: Instant,
    ) -> Result<WsSessionLease, ReserveWsError> {
        let mut sessions = self.lock().map_err(|_| ReserveWsError::Unavailable)?;
        prune(&mut sessions, now);
        let Some(session) = sessions.get_mut(nonce) else {
            return Err(ReserveWsError::UnknownSession);
        };
        if session.active_ws_count >= MAX_WS_CONNECTIONS_PER_SESSION {
            return Err(ReserveWsError::ConnectionLimit);
        }
        session.active_ws_count += 1;
        session.last_activity = now;
        drop(sessions);
        Ok(WsSessionLease {
            registry: Arc::clone(self),
            nonce: nonce.to_owned(),
        })
    }

    pub(crate) fn locale(&self, nonce: &str) -> Result<Option<PublicLocale>, DocumentSessionError> {
        self.locale_at(nonce, Instant::now())
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.sessions
            .lock()
            .map(|sessions| sessions.len())
            .unwrap_or_default()
    }

    fn locale_at(
        &self,
        nonce: &str,
        now: Instant,
    ) -> Result<Option<PublicLocale>, DocumentSessionError> {
        let mut sessions = self.lock()?;
        prune(&mut sessions, now);
        Ok(sessions.get_mut(nonce).map(|session| {
            session.last_activity = now;
            session.locale
        }))
    }

    fn lock(
        &self,
    ) -> Result<MutexGuard<'_, BTreeMap<String, DocumentSession>>, DocumentSessionError> {
        match self.sessions.lock() {
            Ok(sessions) => Ok(sessions),
            Err(_) => {
                tracing::error!(code = "PUBLIC_DOCUMENT_STATE_UNAVAILABLE");
                Err(DocumentSessionError::Unavailable)
            }
        }
    }

    #[cfg(test)]
    fn with_capacity(maximum_sessions: usize) -> Self {
        Self {
            sessions: Mutex::new(BTreeMap::new()),
            maximum_sessions,
        }
    }

    #[cfg(test)]
    fn active_ws_count(&self, nonce: &str) -> Option<u32> {
        self.sessions
            .lock()
            .ok()
            .and_then(|sessions| sessions.get(nonce).map(|session| session.active_ws_count))
    }
}

fn prune(sessions: &mut BTreeMap<String, DocumentSession>, now: Instant) {
    // Leased sessions never expire: a live WebSocket IS the activity. The
    // lease's Drop touches last_activity, so the 2h window restarts when the
    // last connection closes.
    sessions.retain(|_, session| {
        session.active_ws_count > 0
            || now
                .checked_duration_since(session.last_activity)
                .is_some_and(|age| age <= DOCUMENT_SESSION_TTL)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_negotiation_honors_quality_and_support_boundaries() {
        assert_eq!(negotiate_locale(None), PublicLocale::EnUs);
        assert_eq!(
            negotiate_locale(Some("en-US;q=0.4, zh-Hans;q=0.9")),
            PublicLocale::ZhCn
        );
        assert_eq!(
            negotiate_locale(Some("zh-TW, en;q=0.8")),
            PublicLocale::EnUs
        );
        assert_eq!(
            negotiate_locale(Some("zh-CN;q=0, en-US;q=0.7")),
            PublicLocale::EnUs
        );
    }

    #[test]
    fn every_document_nonce_is_distinct_memory_only_and_expires() {
        let registry = DocumentSessionRegistry::default();
        let now = Instant::now();
        let first = registry
            .issue_at(PublicLocale::ZhCn, now)
            .expect("first session");
        let second = registry
            .issue_at(PublicLocale::EnUs, now)
            .expect("second session");
        assert_ne!(first, second);
        assert_eq!(
            registry
                .locale_at(first.as_str(), now)
                .expect("active session"),
            Some(PublicLocale::ZhCn),
        );
        let refresh_at = now + Duration::from_secs(60 * 60);
        assert_eq!(
            registry
                .locale_at(first.as_str(), refresh_at)
                .expect("sliding refresh"),
            Some(PublicLocale::ZhCn)
        );
        assert_eq!(
            registry
                .locale_at(
                    first.as_str(),
                    refresh_at + DOCUMENT_SESSION_TTL + Duration::from_millis(1)
                )
                .expect("expired lookup"),
            None
        );
    }

    #[test]
    fn leased_sessions_survive_ttl_and_eviction_and_capacity_fails_closed() {
        let registry = Arc::new(DocumentSessionRegistry::with_capacity(1));
        let now = Instant::now();
        let nonce = registry
            .issue_at(PublicLocale::EnUs, now)
            .expect("leased session");
        let lease = registry
            .reserve_ws_at(nonce.as_str(), now)
            .expect("reserve one connection");

        // TTL immunity: far past the 2h window the leased session survives.
        let far_future = now + DOCUMENT_SESSION_TTL + Duration::from_secs(3_600);
        assert_eq!(
            registry
                .locale_at(nonce.as_str(), far_future)
                .expect("leased lookup"),
            Some(PublicLocale::EnUs),
            "a live WebSocket lease must keep the nonce alive",
        );

        // Capacity fails closed while everything is leased: 503, never a
        // silent eviction of an active session.
        assert_eq!(
            registry.issue_at(PublicLocale::ZhCn, far_future),
            Err(DocumentSessionError::CapacityExhausted),
        );
        assert!(matches!(
            registry.validate_or_renew_at("unknown-nonce", PublicLocale::ZhCn, far_future),
            Err(DocumentSessionError::CapacityExhausted),
        ));

        // Dropping the lease releases the pin: the session becomes evictable
        // again and admission resumes.
        drop(lease);
        assert_eq!(registry.active_ws_count(nonce.as_str()), Some(0));
        registry
            .issue_at(PublicLocale::ZhCn, far_future + Duration::from_secs(1))
            .expect("unleased sessions are evictable again");
    }

    #[test]
    fn validate_touches_and_renew_signs_new_atomically() {
        let registry = Arc::new(DocumentSessionRegistry::default());
        let now = Instant::now();
        let nonce = registry
            .issue_at(PublicLocale::ZhCn, now)
            .expect("issued session");

        // Valid: the activity window slides.
        let later = now + DOCUMENT_SESSION_TTL - Duration::from_secs(1);
        assert!(matches!(
            registry.validate_or_renew_at(nonce.as_str(), PublicLocale::ZhCn, later),
            Ok(ValidateOutcome::Valid),
        ));
        assert_eq!(
            registry
                .locale_at(
                    nonce.as_str(),
                    later + DOCUMENT_SESSION_TTL - Duration::from_secs(1)
                )
                .expect("slid window"),
            Some(PublicLocale::ZhCn),
        );

        // Renew: an unknown nonce yields a usable replacement; the supplied
        // one stays invalid (nothing resurrects it).
        let Ok(ValidateOutcome::Renewed(renewed)) = registry.validate_or_renew_at(
            "00000000-0000-4000-8000-00000000dead",
            PublicLocale::EnUs,
            later,
        ) else {
            panic!("unknown nonce must renew");
        };
        assert_eq!(
            registry
                .locale_at(renewed.as_str(), later)
                .expect("renewed lookup"),
            Some(PublicLocale::EnUs),
        );
        assert_eq!(
            registry
                .locale_at("00000000-0000-4000-8000-00000000dead", later)
                .expect("stale lookup"),
            None,
        );
    }

    #[test]
    fn reserve_ws_enforces_the_per_session_cap_and_unknown_nonces() {
        let registry = Arc::new(DocumentSessionRegistry::default());
        let now = Instant::now();
        let nonce = registry
            .issue_at(PublicLocale::EnUs, now)
            .expect("issued session");

        assert_eq!(
            registry
                .reserve_ws_at("00000000-0000-4000-8000-00000000dead", now)
                .err(),
            Some(ReserveWsError::UnknownSession),
        );

        let mut leases = Vec::new();
        for _ in 0..MAX_WS_CONNECTIONS_PER_SESSION {
            leases.push(
                registry
                    .reserve_ws_at(nonce.as_str(), now)
                    .expect("within the connection cap"),
            );
        }
        assert_eq!(
            registry.reserve_ws_at(nonce.as_str(), now).err(),
            Some(ReserveWsError::ConnectionLimit),
        );
        leases.pop();
        registry
            .reserve_ws_at(nonce.as_str(), now)
            .expect("a dropped lease frees one slot");
    }

    #[test]
    fn full_registry_evicts_the_least_recent_document_instead_of_refusing_new_pages() {
        let registry = DocumentSessionRegistry::with_capacity(1);
        let now = Instant::now();
        let first = registry
            .issue_at(PublicLocale::ZhCn, now)
            .expect("first session");
        let second = registry
            .issue_at(PublicLocale::EnUs, now + Duration::from_millis(1))
            .expect("new page remains available at capacity");
        assert_eq!(
            registry
                .locale_at(first.as_str(), now + Duration::from_millis(1))
                .expect("evicted lookup"),
            None
        );
        assert_eq!(
            registry
                .locale_at(second.as_str(), now + Duration::from_millis(1))
                .expect("second lookup"),
            Some(PublicLocale::EnUs)
        );
    }
}
