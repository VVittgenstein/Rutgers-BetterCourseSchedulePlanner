use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use bcsp_application::SessionNonce;
use serde::Serialize;
use thiserror::Error;

const DOCUMENT_SESSION_TTL: Duration = Duration::from_secs(2 * 60 * 60);
const MAX_DOCUMENT_SESSIONS: usize = 4_096;

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
        if sessions.len() >= self.maximum_sessions {
            let oldest = sessions
                .iter()
                .min_by(|(left_key, left), (right_key, right)| {
                    left.last_activity
                        .cmp(&right.last_activity)
                        .then_with(|| left_key.cmp(right_key))
                })
                .map(|(key, _)| key.clone());
            if let Some(oldest) = oldest {
                sessions.remove(&oldest);
            }
        }
        let nonce = SessionNonce::generate();
        sessions.insert(
            nonce.as_str().to_owned(),
            DocumentSession {
                locale,
                last_activity: now,
            },
        );
        Ok(nonce)
    }

    pub(crate) fn locale(&self, nonce: &str) -> Result<Option<PublicLocale>, DocumentSessionError> {
        self.locale_at(nonce, Instant::now())
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
}

fn prune(sessions: &mut BTreeMap<String, DocumentSession>, now: Instant) {
    sessions.retain(|_, session| {
        now.checked_duration_since(session.last_activity)
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
