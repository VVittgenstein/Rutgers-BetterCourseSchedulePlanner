//! Local watch wiring: the admission source the shared watch manager
//! consults, and the local decorator that keeps the browser's watch socket
//! from being a second source of truth about what is monitored.
//!
//! The presence path is frozen here so the S2b pin -- an un-injected path
//! must 404 -- and the eventual registration cannot drift apart: the shared
//! host promises nothing about a path it was never given, and the 404 is the
//! local extension fallback's behaviour. The desired-watch path is NOT here:
//! it is an ordinary local HTTP resource (see `crate::desired`), and a page
//! that tries to open a socket on it reaches the HTTP handler.

use std::collections::BTreeMap;
use std::num::NonZeroU8;
use std::sync::{Arc, Mutex};

use bcsp_application::{
    OpenRuntimeSnapshotRegistry, OutboundSender, SharedWatchSocket, WatchAdmissionSource,
    WebSocketExtension, is_product_campus,
};
use bcsp_contracts::{
    SectionKey, TermCampusKey, TraceId, WatchClientCommandV1, WsClientEnvelope,
    decode_versioned_envelope_json,
};
use bcsp_domain::{RutgersTermWindow, RutgersTermWindowScope};
use bcsp_local_user_state::PersonalStateStore;
use bcsp_open::{
    OpenProjectionError, project_current_open_observation, project_current_open_observations,
};
use bcsp_watch::{WatchManagerError, WatchStartAdmission};
use time::OffsetDateTime;

use crate::{
    DesiredWatchCoordinator, LocalConsole, LocalConsoleEvent, LocalPrimaryDatabase,
    LocalRuntimeCore, history::LocalWatchHistorySink,
};

/// Frozen path of the local presence route.
pub const LOCAL_PRESENCE_SOCKET_PATH: &str = "/api/v1/local/presence";

/// The local process opts into the complete count domain carried by the v1
/// watch wire. Public/default compositions intentionally retain their cap of
/// nine.
pub const LOCAL_MAX_ACTIVE_WATCHES: u8 = u8::MAX;
const _: () =
    assert!(bcsp_local_user_state::MAX_DESIRED_WATCHES == LOCAL_MAX_ACTIVE_WATCHES as usize);

struct LocalWatchAdmission {
    database: Arc<Mutex<LocalPrimaryDatabase>>,
    runtime: LocalRuntimeCore,
    open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
}

impl WatchAdmissionSource for LocalWatchAdmission {
    fn admission_for(&self, section: &SectionKey) -> WatchStartAdmission {
        if !self.target_supported(section) {
            return WatchStartAdmission::UnsupportedTarget;
        }
        if !self.term_in_range(section) {
            return WatchStartAdmission::TermOutOfRange;
        }
        let target = section.target();
        let Ok(snapshot) = self.open_runtime.snapshot(&target) else {
            return WatchStartAdmission::TargetUnavailable;
        };
        let Ok(runtime) = self.runtime.projection_runtime(&snapshot) else {
            return WatchStartAdmission::TargetUnavailable;
        };
        let Ok(mut database) = self.database.lock() else {
            return WatchStartAdmission::TargetUnavailable;
        };
        admission_from_projection(project_current_open_observation(
            database.operational_mut(),
            section,
            &runtime,
        ))
    }

    fn admissions_for(&self, sections: &[SectionKey]) -> Vec<WatchStartAdmission> {
        let mut admissions = vec![WatchStartAdmission::TargetUnavailable; sections.len()];
        let mut targets = BTreeMap::<TermCampusKey, Vec<(usize, SectionKey)>>::new();
        for (position, section) in sections.iter().enumerate() {
            if !self.target_supported(section) {
                admissions[position] = WatchStartAdmission::UnsupportedTarget;
            } else if !self.term_in_range(section) {
                admissions[position] = WatchStartAdmission::TermOutOfRange;
            } else {
                targets
                    .entry(section.target())
                    .or_default()
                    .push((position, section.clone()));
            }
        }

        for (target, members) in targets {
            let projection = (|| {
                let snapshot = self.open_runtime.snapshot(&target).ok()?;
                let runtime = self.runtime.projection_runtime(&snapshot).ok()?;
                let mut database = self.database.lock().ok()?;
                let sections = members
                    .iter()
                    .map(|(_, section)| section.clone())
                    .collect::<Vec<_>>();
                project_current_open_observations(
                    database.operational_mut(),
                    &target,
                    &sections,
                    &runtime,
                )
                .ok()
            })();

            let Some(projection) = projection else {
                continue;
            };
            for (position, section) in members {
                admissions[position] = match projection.get(&section) {
                    Some(observation) => WatchStartAdmission::admitted(observation.clone()),
                    None => WatchStartAdmission::SectionNotFound,
                };
            }
        }
        admissions
    }

    fn target_supported(&self, section: &SectionKey) -> bool {
        is_product_campus(section.campus().as_str())
    }

    fn term_in_range(&self, section: &SectionKey) -> bool {
        watch_term_in_range(section)
    }
}

#[cfg(test)]
fn watch_target_supported(section: &SectionKey) -> bool {
    is_product_campus(section.campus().as_str())
}

fn watch_term_in_range(section: &SectionKey) -> bool {
    RutgersTermWindow::at(OffsetDateTime::now_utc(), RutgersTermWindowScope::Public).is_ok_and(
        |window| section.term() == window.current_term() || section.term() == window.next_term(),
    )
}

fn admission_from_projection(
    projection: Result<Option<bcsp_contracts::OpenObservationV1>, OpenProjectionError>,
) -> WatchStartAdmission {
    match projection {
        Ok(observation) => WatchStartAdmission::admitted(observation),
        Err(OpenProjectionError::SectionNotPublished) => WatchStartAdmission::SectionNotFound,
        Err(_) => WatchStartAdmission::TargetUnavailable,
    }
}

pub fn create_local_watch_socket(
    database: Arc<Mutex<LocalPrimaryDatabase>>,
    history_store: PersonalStateStore,
    runtime: LocalRuntimeCore,
    open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
) -> Result<Arc<SharedWatchSocket>, WatchManagerError> {
    create_local_watch_socket_with_history(database, history_store, runtime, open_runtime)
        .map(|(socket, _)| socket)
}

/// Like [`create_local_watch_socket`], also returning the history sink so
/// the runtime can ask its worker thread about the connection it owns.
pub(crate) fn create_local_watch_socket_with_history(
    database: Arc<Mutex<LocalPrimaryDatabase>>,
    history_store: PersonalStateStore,
    runtime: LocalRuntimeCore,
    open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
) -> Result<(Arc<SharedWatchSocket>, Arc<LocalWatchHistorySink>), WatchManagerError> {
    let admission: Arc<dyn WatchAdmissionSource> = Arc::new(LocalWatchAdmission {
        database: database.clone(),
        runtime,
        open_runtime,
    });
    let history = Arc::new(LocalWatchHistorySink::new(history_store));
    let socket = Arc::new(SharedWatchSocket::try_new_with_max_active_watches(
        admission,
        history.clone(),
        NonZeroU8::new(LOCAL_MAX_ACTIVE_WATCHES).expect("the local watch limit is nonzero"),
    )?);
    Ok((socket, history))
}

/// The local build's watch socket.
///
/// Wraps the shared socket rather than replacing it, and changes exactly two
/// things, both of which follow from the local build having durable intent
/// that the public build does not:
///
/// 1. `START_WATCH`, `STOP_WATCH` and `UPDATE_POLICY` are refused. Locally,
///    what is monitored is decided by `desired_watches` over HTTP. A frame
///    that could start or stop a watch would be a second source of truth,
///    and the two would disagree the first time a page sent one -- the watch
///    would be running with no row behind it, so a restart would silently
///    lose it, and the authority read would show it as not monitored while
///    it was. Refusing is fail-closed: the page is told nothing happened,
///    because nothing did.
/// 2. The five episode commands and the acknowledge-all command are executed
///    against the OWNER connection, because that is where the watches live.
///    Routing them to the sending connection would answer "unknown episode"
///    for every alert the user is actually looking at.
///
/// The public build injects the shared socket directly and is untouched by
/// any of this: its watches are connection-scoped, and its pages still
/// start and stop them over the wire.
pub struct LocalWatchRoute {
    watch: Arc<SharedWatchSocket>,
    coordinator: Arc<DesiredWatchCoordinator>,
    console: Option<Arc<LocalConsole>>,
}

impl LocalWatchRoute {
    pub const fn new(
        watch: Arc<SharedWatchSocket>,
        coordinator: Arc<DesiredWatchCoordinator>,
    ) -> Self {
        Self {
            watch,
            coordinator,
            console: None,
        }
    }

    #[must_use]
    pub fn with_console(mut self, console: Arc<LocalConsole>) -> Self {
        self.console = Some(console);
        self
    }

    fn report(&self, event: &LocalConsoleEvent) {
        if let Some(console) = &self.console {
            console.report(event);
        }
    }

    fn reconcile_audience(&self) {
        if let Err(error) = self.coordinator.audience_changed() {
            tracing::warn!(
                ?error,
                "desired-watch reconcile after an audience change failed"
            );
        }
    }
}

impl WebSocketExtension for LocalWatchRoute {
    fn connect(&self, connection_id: TraceId, outbound: OutboundSender) -> bool {
        if !self.watch.connect(connection_id, outbound) {
            return false;
        }
        // The first page to attach is what brings the stored intent back to
        // life, so this is the restore path after a restart as much as it is
        // the attach path.
        self.reconcile_audience();
        self.report(&LocalConsoleEvent::AlertsAttached);
        true
    }

    fn transport_activity(&self, connection_id: TraceId) {
        self.watch.transport_activity(connection_id);
    }

    fn receive_text(&self, connection_id: TraceId, message: &str) {
        let Ok(envelope) = decode_versioned_envelope_json::<WsClientEnvelope<WatchClientCommandV1>>(
            message.as_bytes(),
        ) else {
            // Let the shared socket log the rejection the one way it always
            // has; it will decode the same bytes and refuse them again.
            self.watch.receive_text(connection_id, message);
            return;
        };
        match envelope.payload() {
            WatchClientCommandV1::StartWatch { .. }
            | WatchClientCommandV1::StopWatch { .. }
            | WatchClientCommandV1::UpdatePolicy { .. } => {
                tracing::warn!(
                    "refused a legacy watch mutation on the local socket; desired-watch \
                     intent is submitted over HTTP",
                );
            }
            WatchClientCommandV1::AcknowledgeEpisode { .. }
            | WatchClientCommandV1::AcknowledgeAllEpisodes {}
            | WatchClientCommandV1::ResumeTimedOutEpisode { .. }
            | WatchClientCommandV1::ResetAudibleCount { .. }
            | WatchClientCommandV1::ReportCueOutcome { .. }
            | WatchClientCommandV1::DismissAlert { .. } => {
                if let Err(error) = self.watch.owner_command(envelope.into_payload()) {
                    tracing::warn!(?error, "rejected a local episode command");
                }
            }
            WatchClientCommandV1::HeartbeatAck { .. } => {
                self.watch.receive_text(connection_id, message);
            }
        }
    }

    fn disconnect(&self, connection_id: TraceId) {
        self.watch.disconnect(connection_id);
        // The last page leaving tears the physical watches down and keeps
        // every row: closing a tab is not the user changing their mind.
        self.reconcile_audience();
        if self.watch.audience_connection_count() == 0 {
            self.report(&LocalConsoleEvent::AlertsDetached);
        }
    }

    fn tick(&self) {
        self.watch.tick();
        self.coordinator.tick();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bcsp_contracts::SectionKey;

    #[test]
    fn admission_distinguishes_missing_section_and_missing_target() {
        assert_eq!(
            admission_from_projection(Ok(None)),
            WatchStartAdmission::admitted(None)
        );
        assert_eq!(
            admission_from_projection(Err(OpenProjectionError::SectionNotPublished)),
            WatchStartAdmission::SectionNotFound
        );
        assert_eq!(
            admission_from_projection(Err(OpenProjectionError::TargetNotPublished)),
            WatchStartAdmission::TargetUnavailable
        );
    }

    #[test]
    fn watch_targets_are_limited_to_the_three_product_campuses() {
        for campus in ["NB", "NK", "CM"] {
            let section = SectionKey::try_new("92026", campus, "12345").expect("section");
            assert!(watch_target_supported(&section), "{campus}");
        }
        for campus in ["NWK", "CAM", "ONLINE_NB"] {
            let section = SectionKey::try_new("92026", campus, "12345").expect("section");
            assert!(!watch_target_supported(&section), "{campus}");
        }
    }
}
