//! Page presence, and the countdown that follows the last page out.
//!
//! The local build is a program the user starts by opening a page. When every
//! page is gone there is nobody to ring, and a process that keeps polling
//! Rutgers for a browser nobody has open is doing work for no one. So the
//! runtime waits a minute and exits.
//!
//! Presence is deliberately its OWN connection, and not the watch socket. A
//! watch socket only exists once the user has started watching something; a
//! page that is merely browsing has none. Counting watch connections would
//! mean a user reading a course list watched the program exit under them.
//!
//! It is equally deliberately not part of the desired-watch audience. Watch
//! Disconnect is a statement about alerts; closing the last tab is a
//! statement about the program. The two are counted separately because they
//! answer different questions, and one of them tears down physical watches
//! while the other does not.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use bcsp_application::{OutboundSender, WebSocketExtension};
use bcsp_contracts::{TraceId, WsClientEnvelope, WsServerEnvelope, decode_versioned_envelope_json};
use serde::{Deserialize, Serialize};

/// How long the program stays alive after the last page leaves.
///
/// A product number, not a technical one: it is long enough to cover a
/// refresh, a browser restart, and a user closing one window to open another,
/// and short enough that a program nobody is using does not sit there.
pub const LOCAL_IDLE_EXIT_COUNTDOWN: Duration = Duration::from_secs(60);

/// How long a connection has to say which tab it is before it is dropped.
///
/// A socket that never identifies itself is not a page: it is an open
/// connection with nothing behind it, and counting it would keep the program
/// alive on the strength of something that cannot ring.
pub const LOCAL_PRESENCE_HELLO_DEADLINE: Duration = Duration::from_secs(10);

/// The frame a page sends to say which tab it is. Local-only by construction:
/// this type is not in `bcsp-contracts`, so the public build cannot name it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum LocalPresenceCommandV1 {
    /// The first frame, and the only one that ever registers a connection.
    #[serde(rename_all = "camelCase")]
    Hello { tab_id: TraceId },
}

/// What the runtime answers a page that registered.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LocalPresenceEventV1 {
    /// This tab is counted. The page shows nothing for it; it is evidence.
    #[serde(rename_all = "camelCase")]
    Registered { tab_id: TraceId, pages: u64 },
}

/// What the countdown is doing, as one value.
///
/// `generation` is what makes a late expiry safe. Between scheduling the
/// countdown and its due time a page can come back, and an expiry derived
/// from the old count would exit the program while somebody is looking at it.
/// A countdown whose generation has moved on is dropped, not applied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalPresencePhase {
    /// At least one page is here.
    Running,
    /// No page is here, and the program will exit at `due_at` if none returns.
    CountingDown { generation: u64, due_at: Instant },
    /// The exit has been asked for. Nothing reverses this.
    Exiting,
}

/// A snapshot of what the runtime would say about presence right now.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalPresenceStateV1 {
    pub pages: u64,
    pub phase: LocalPresencePhase,
}

struct Connection {
    /// When this connection must have identified itself by.
    hello_deadline: Instant,
    /// The tab it claimed, once it has.
    tab: Option<TraceId>,
    outbound: OutboundSender,
}

struct Registry {
    connections: HashMap<TraceId, Connection>,
    /// Which connection currently speaks for a tab.
    ///
    /// Keyed by tab rather than by connection because a tab that reconnects
    /// is the SAME page: the replacement takes the binding, and the old
    /// connection's `disconnect` -- which can arrive afterwards -- must not
    /// remove a binding that no longer belongs to it.
    tabs: HashMap<TraceId, TraceId>,
    phase: LocalPresencePhase,
    /// Whether a page has ever been counted on this runtime.
    ///
    /// The countdown says every page has CLOSED, and before the first one
    /// opens nothing has. A runtime that started counting at launch would
    /// open its console with an alarming line about pages that never existed,
    /// and would race the browser it just asked the operating system to open.
    ever_counted_a_page: bool,
    generation: u64,
    /// The whole second the countdown last reported, so the console shows a
    /// countdown rather than a four-times-a-second stutter.
    announced_second: Option<u64>,
    sealed: bool,
}

impl Registry {
    fn pages(&self) -> u64 {
        u64::try_from(self.tabs.len()).unwrap_or(u64::MAX)
    }
}

/// The local presence route.
///
/// Registered on the shared host's validated secondary-route set, so it
/// inherits the same admission the watch route has: exact Host, exact Origin,
/// the session nonce, and the shared subprotocol.
pub struct LocalPresenceRoute {
    registry: Mutex<Registry>,
    countdown: Duration,
    hello_deadline: Duration,
    request_exit: Box<dyn Fn() + Send + Sync>,
    /// Where the user sees this happen. Presence is the one subsystem whose
    /// events are entirely about the person at the keyboard -- a page opened,
    /// a page closed, the program is about to exit -- so it reports them.
    console: Option<std::sync::Arc<crate::LocalConsole>>,
}

impl LocalPresenceRoute {
    pub fn new(request_exit: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            registry: Mutex::new(Registry {
                connections: HashMap::new(),
                tabs: HashMap::new(),
                phase: LocalPresencePhase::Running,
                ever_counted_a_page: false,
                generation: 0,
                announced_second: None,
                sealed: false,
            }),
            countdown: LOCAL_IDLE_EXIT_COUNTDOWN,
            hello_deadline: LOCAL_PRESENCE_HELLO_DEADLINE,
            request_exit: Box::new(request_exit),
            console: None,
        }
    }

    #[must_use]
    pub fn with_console(mut self, console: std::sync::Arc<crate::LocalConsole>) -> Self {
        self.console = Some(console);
        self
    }

    fn report(&self, event: &crate::LocalConsoleEvent) {
        if let Some(console) = &self.console {
            console.report(event);
        }
    }

    /// Replaces the idle countdown. Tests only: the product number is
    /// [`LOCAL_IDLE_EXIT_COUNTDOWN`], and a test that waited it out would be
    /// proving the clock rather than the state machine.
    #[must_use]
    pub const fn with_countdown(mut self, countdown: Duration) -> Self {
        self.countdown = countdown;
        self
    }

    /// Replaces the registration deadline, for the same reason.
    #[must_use]
    pub const fn with_hello_deadline(mut self, deadline: Duration) -> Self {
        self.hello_deadline = deadline;
        self
    }

    pub fn state(&self) -> LocalPresenceStateV1 {
        let registry = self.lock();
        LocalPresenceStateV1 {
            pages: registry.pages(),
            phase: registry.phase,
        }
    }

    /// Stops the countdown for good, for an exit that is already happening.
    ///
    /// Called from the ordered shutdown so a countdown cannot fire a second
    /// exit request into a runtime that is already tearing itself down.
    pub fn seal(&self) {
        let mut registry = self.lock();
        registry.sealed = true;
        registry.phase = LocalPresencePhase::Exiting;
        registry.connections.clear();
        registry.tabs.clear();
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Registry> {
        // A poisoned presence lock must not take the program down or, worse,
        // silently stop counting. The state behind it is a page count and a
        // deadline; recovering it is strictly better than exiting on it.
        self.registry
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Re-evaluates the countdown after anything that can change the count.
    ///
    /// The count is read from the live registry every time rather than kept
    /// alongside it: two copies of "how many pages are there" is how a
    /// program exits with a page open.
    fn reconcile(&self, registry: &mut Registry, now: Instant) -> bool {
        if registry.sealed {
            return false;
        }
        let pages = registry.pages();
        match registry.phase {
            LocalPresencePhase::Exiting => false,
            LocalPresencePhase::Running => {
                if pages > 0 {
                    return false;
                }
                if !registry.ever_counted_a_page {
                    // Nothing has closed, because nothing has opened. The
                    // runtime waits for the page the launcher is opening.
                    return false;
                }
                registry.generation += 1;
                registry.announced_second = None;
                registry.phase = LocalPresencePhase::CountingDown {
                    generation: registry.generation,
                    due_at: now + self.countdown,
                };
                tracing::info!(
                    seconds = self.countdown.as_secs(),
                    "every page has closed; the runtime will exit unless one returns"
                );
                self.report(&crate::LocalConsoleEvent::ExitCountdown {
                    seconds: self.countdown.as_secs(),
                });
                false
            }
            LocalPresencePhase::CountingDown { generation, due_at } => {
                if pages > 0 {
                    // A page came back. The countdown that was running is
                    // invalidated by the generation bump, so an expiry
                    // already on its way cannot apply.
                    registry.generation += 1;
                    registry.announced_second = None;
                    registry.phase = LocalPresencePhase::Running;
                    tracing::info!(pages, "a page returned; the exit countdown is cancelled");
                    self.report(&crate::LocalConsoleEvent::ExitCancelled { pages });
                    return false;
                }
                if now < due_at {
                    let remaining = (due_at - now).as_secs();
                    if registry.announced_second != Some(remaining) {
                        registry.announced_second = Some(remaining);
                        tracing::info!(
                            seconds = remaining,
                            "no page is open; exiting shortly unless one returns"
                        );
                        // Once a second, not once a tick. The maintenance
                        // timer runs four times a second, and a console that
                        // repeated itself four times a second would bury
                        // every other line in the window.
                        self.report(&crate::LocalConsoleEvent::ExitCountdown {
                            seconds: remaining,
                        });
                    }
                    return false;
                }
                // Due. The count was re-read at the top of this function from
                // the live registry, and the generation still matches the one
                // this countdown started under -- both have to hold, because
                // a page that came back and left again is a DIFFERENT
                // countdown that has not run out yet.
                if generation != registry.generation {
                    return false;
                }
                registry.phase = LocalPresencePhase::Exiting;
                tracing::info!("no page returned; shutting the runtime down");
                true
            }
        }
    }

    fn after_change(&self, now: Instant) {
        let exit = {
            let mut registry = self.lock();
            self.reconcile(&mut registry, now)
        };
        // Outside the lock: the trigger is a watch-channel send that the
        // lifecycle listens on, and holding a lock across it buys nothing.
        if exit {
            (self.request_exit)();
        }
    }

    fn send(connection: &Connection, event: &LocalPresenceEventV1, message_id: TraceId) {
        let envelope = WsServerEnvelope::new(message_id, event);
        if let Ok(frame) = serde_json::to_string(&envelope) {
            let _ = connection.outbound.send(frame);
        }
    }
}

impl WebSocketExtension for LocalPresenceRoute {
    fn connect(&self, connection_id: TraceId, outbound: OutboundSender) -> bool {
        let now = Instant::now();
        let mut registry = self.lock();
        if registry.sealed || registry.phase == LocalPresencePhase::Exiting {
            // The exit has been asked for and the ordered shutdown is under
            // way. Accepting a page here would count it, answer REGISTERED,
            // and then take the runtime away from it a moment later.
            return false;
        }
        registry.connections.insert(
            connection_id,
            Connection {
                hello_deadline: now + self.hello_deadline,
                tab: None,
                outbound,
            },
        );
        // Not counted yet. A connection counts when it says which tab it is,
        // and not before -- otherwise anything that can open a socket can
        // keep the program alive.
        true
    }

    fn transport_activity(&self, _connection_id: TraceId) {
        // Deliberately nothing. Ping and Pong are the transport keeping
        // itself alive; they say nothing about a page having identified
        // itself, and letting them push the registration deadline out would
        // let a socket that never says HELLO live forever.
    }

    fn receive_text(&self, connection_id: TraceId, message: &str) {
        let Ok(envelope) = decode_versioned_envelope_json::<
            WsClientEnvelope<LocalPresenceCommandV1>,
        >(message.as_bytes()) else {
            self.close(connection_id, "presence frame could not be decoded");
            return;
        };
        let message_id = envelope.message_id();
        let LocalPresenceCommandV1::Hello { tab_id } = envelope.into_payload();
        let now = Instant::now();
        let exit = {
            let mut registry = self.lock();
            if registry.sealed || registry.phase == LocalPresencePhase::Exiting {
                // The exit decision has already committed under this same
                // lock. This socket may have been legally admitted while the
                // countdown was still running, but registering it now would
                // answer REGISTERED to a page the runtime is about to take
                // away -- and nothing reverses Exiting. Refused the same way
                // a socket refused at admission is: closed, not counted, not
                // reported as a page.
                drop(registry);
                self.close(connection_id, "presence HELLO arrived after the exit decision");
                return;
            }
            let Some(connection) = registry.connections.get(&connection_id) else {
                return;
            };
            if connection.tab.is_some() {
                // A second HELLO on the same connection. One connection is
                // one tab; a connection that claims two has either been
                // tampered with or is confused, and either way what it says
                // about presence cannot be trusted.
                drop(registry);
                self.close(connection_id, "presence connection sent a second identity");
                return;
            }
            if let Some(connection) = registry.connections.get_mut(&connection_id) {
                connection.tab = Some(tab_id);
            }
            // The tab's binding moves to this connection. A page that
            // reconnected is the same page; the connection it replaced is
            // already gone or about to be, and its late disconnect is
            // handled by comparing identity rather than presence.
            registry.tabs.insert(tab_id, connection_id);
            registry.ever_counted_a_page = true;
            let pages = registry.pages();
            if let Some(connection) = registry.connections.get(&connection_id) {
                Self::send(
                    connection,
                    &LocalPresenceEventV1::Registered { tab_id, pages },
                    message_id,
                );
            }
            tracing::info!(pages, "a page is open");
            self.report(&crate::LocalConsoleEvent::PageOpened { pages });
            self.reconcile(&mut registry, now)
        };
        if exit {
            (self.request_exit)();
        }
    }

    fn disconnect(&self, connection_id: TraceId) {
        let now = Instant::now();
        let exit = {
            let mut registry = self.lock();
            let Some(connection) = registry.connections.remove(&connection_id) else {
                return;
            };
            if let Some(tab) = connection.tab {
                // Only if this connection is still the one speaking for the
                // tab. A disconnect from a connection the tab has already
                // replaced would otherwise delete a page that is right there.
                if registry.tabs.get(&tab) == Some(&connection_id) {
                    registry.tabs.remove(&tab);
                    let pages = registry.pages();
                    tracing::info!(pages, "a page closed");
                    self.report(&crate::LocalConsoleEvent::PageClosed { pages });
                }
            }
            self.reconcile(&mut registry, now)
        };
        if exit {
            (self.request_exit)();
        }
    }

    fn tick(&self) {
        let now = Instant::now();
        let expired: Vec<TraceId> = {
            let registry = self.lock();
            registry
                .connections
                .iter()
                .filter(|(_, connection)| {
                    connection.tab.is_none() && connection.hello_deadline <= now
                })
                .map(|(id, _)| *id)
                .collect()
        };
        for connection_id in expired {
            // Re-checked under the lock inside `close_unidentified`. The list
            // was taken a moment ago, and a HELLO that was merely slow can
            // arrive in between: closing on the stale reading would drop a
            // page that had just identified itself, and the count would go to
            // zero under a tab that is right there.
            self.close_unidentified(connection_id);
        }
        self.after_change(now);
    }
}

impl LocalPresenceRoute {
    /// Ends a connection by dropping the only thing that keeps its pump alive.
    ///
    /// The shared transport has no close-with-reason primitive: a connection
    /// ends when its outbound sender is gone, and the pump then reports the
    /// disconnect through the ordinary path. Removing the record here as well
    /// keeps a refused connection from counting in the window before that
    /// happens.
    /// Closes a connection that has still not said which tab it is.
    ///
    /// The check and the removal happen under one lock, so a HELLO that
    /// landed after the sweep read the registry keeps its connection.
    fn close_unidentified(&self, connection_id: TraceId) {
        let now = Instant::now();
        let exit = {
            let mut registry = self.lock();
            match registry.connections.get(&connection_id) {
                Some(connection) if connection.tab.is_none() => {}
                _ => return,
            }
            registry.connections.remove(&connection_id);
            tracing::warn!(
                reason = "presence connection never identified a tab",
                "closed a local presence connection",
            );
            self.reconcile(&mut registry, now)
        };
        if exit {
            (self.request_exit)();
        }
    }

    fn close(&self, connection_id: TraceId, reason: &'static str) {
        let now = Instant::now();
        let exit = {
            let mut registry = self.lock();
            let Some(connection) = registry.connections.remove(&connection_id) else {
                return;
            };
            let closed_a_page = if let Some(tab) = connection.tab
                && registry.tabs.get(&tab) == Some(&connection_id)
            {
                registry.tabs.remove(&tab);
                true
            } else {
                false
            };
            drop(connection);
            tracing::warn!(reason, "closed a local presence connection");
            if closed_a_page {
                // The user lost a page here just as surely as if the browser
                // had closed it, and the console count would otherwise drift
                // from what the runtime actually believes.
                self.report(&crate::LocalConsoleEvent::PageClosed {
                    pages: registry.pages(),
                });
            }
            self.reconcile(&mut registry, now)
        };
        if exit {
            (self.request_exit)();
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::*;

    fn trace(value: u64) -> TraceId {
        format!("00000000-0000-4000-8000-{value:012x}")
            .parse()
            .expect("trace id")
    }

    struct Harness {
        route: LocalPresenceRoute,
        exits: std::sync::Arc<Mutex<u32>>,
    }

    fn harness(countdown: Duration) -> Harness {
        harness_with(countdown, Duration::from_millis(60))
    }

    fn harness_with(countdown: Duration, hello_deadline: Duration) -> Harness {
        let exits = std::sync::Arc::new(Mutex::new(0));
        let counter = exits.clone();
        let route = LocalPresenceRoute::new(move || {
            *counter.lock().expect("exit counter") += 1;
        })
        .with_countdown(countdown)
        .with_hello_deadline(hello_deadline);
        Harness { route, exits }
    }

    fn attach(route: &LocalPresenceRoute, id: u64) -> mpsc::UnboundedReceiver<String> {
        let (outbound, inbound) = OutboundSender::unbounded_pair();
        assert!(route.connect(trace(id), outbound));
        inbound
    }

    fn hello(route: &LocalPresenceRoute, connection: u64, tab: u64) {
        route.receive_text(
            trace(connection),
            &serde_json::to_string(&WsClientEnvelope::new(
                trace(9_000 + connection),
                LocalPresenceCommandV1::Hello { tab_id: trace(tab) },
            ))
            .expect("hello frame"),
        );
    }

    #[test]
    fn the_production_countdown_is_sixty_seconds() {
        assert_eq!(LOCAL_IDLE_EXIT_COUNTDOWN, Duration::from_secs(60));
    }

    #[test]
    fn a_connection_counts_only_after_it_says_which_tab_it_is() {
        let harness = harness(Duration::from_millis(50));
        let _inbound = attach(&harness.route, 1);
        assert_eq!(harness.route.state().pages, 0);

        hello(&harness.route, 1, 100);
        assert_eq!(harness.route.state().pages, 1);
        assert_eq!(harness.route.state().phase, LocalPresencePhase::Running);
    }

    #[test]
    fn ping_and_pong_do_not_extend_the_registration_deadline() {
        let harness = harness(Duration::from_secs(30));
        let mut inbound = attach(&harness.route, 1);

        // The transport keeps itself alive for the whole deadline. That is
        // the transport's business; it is not a page saying it is there.
        for _ in 0..5 {
            harness.route.transport_activity(trace(1));
            std::thread::sleep(Duration::from_millis(20));
        }
        harness.route.tick();

        assert_eq!(harness.route.state().pages, 0);
        assert!(inbound.try_recv().is_err(), "an unidentified socket is closed, not answered");
    }

    #[test]
    fn a_second_identity_on_one_connection_closes_it_and_does_not_count() {
        let harness = harness(Duration::from_secs(30));
        let _inbound = attach(&harness.route, 1);
        hello(&harness.route, 1, 100);
        assert_eq!(harness.route.state().pages, 1);

        hello(&harness.route, 1, 101);
        // The connection is gone and neither tab is counted: what it said
        // about itself cannot be relied on.
        assert_eq!(harness.route.state().pages, 0);
    }

    #[test]
    fn an_undecodable_frame_closes_the_connection() {
        let harness = harness(Duration::from_secs(30));
        let _inbound = attach(&harness.route, 1);
        harness
            .route
            .receive_text(trace(1), "{\"protocolVersion\":1}");
        assert_eq!(harness.route.state().pages, 0);
    }

    #[test]
    fn a_late_disconnect_does_not_evict_the_connection_that_replaced_it() {
        let harness = harness(Duration::from_secs(30));
        let _first = attach(&harness.route, 1);
        hello(&harness.route, 1, 100);
        let _second = attach(&harness.route, 2);
        hello(&harness.route, 2, 100);
        assert_eq!(harness.route.state().pages, 1);

        // The dead connection's disconnect arrives now. The tab is still
        // open -- on the connection that replaced it.
        harness.route.disconnect(trace(1));
        assert_eq!(harness.route.state().pages, 1);
        assert_eq!(harness.route.state().phase, LocalPresencePhase::Running);
    }

    #[test]
    fn a_runtime_that_has_never_seen_a_page_does_not_count_down() {
        // The launcher opens the browser right after the runtime binds, so
        // there is a window where no page exists yet. Counting down there
        // would open every session with an alarming line about pages that
        // never existed, and would race the browser being opened.
        let harness = harness(Duration::from_millis(40));
        for _ in 0..6 {
            harness.route.tick();
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(harness.route.state().phase, LocalPresencePhase::Running);
        assert_eq!(*harness.exits.lock().expect("exit counter"), 0);

        // Once a page HAS been here, its leaving is the last page leaving.
        let _inbound = attach(&harness.route, 1);
        hello(&harness.route, 1, 100);
        harness.route.disconnect(trace(1));
        assert!(matches!(
            harness.route.state().phase,
            LocalPresencePhase::CountingDown { .. }
        ));
    }

    #[test]
    fn a_page_arriving_after_the_exit_was_asked_for_is_refused() {
        let harness = harness(Duration::from_millis(40));
        let _inbound = attach(&harness.route, 1);
        hello(&harness.route, 1, 100);
        harness.route.disconnect(trace(1));
        std::thread::sleep(Duration::from_millis(60));
        harness.route.tick();
        assert_eq!(*harness.exits.lock().expect("exit counter"), 1);

        // The ordered shutdown is under way. Accepting a page now would count
        // it, answer REGISTERED, and take the runtime away a moment later.
        let (outbound, _rejected) = OutboundSender::unbounded_pair();
        assert!(!harness.route.connect(trace(2), outbound));
        assert_eq!(harness.route.state().pages, 0);
    }

    #[test]
    fn one_of_two_tabs_leaving_is_not_the_last_page() {
        let harness = harness(Duration::from_millis(80));
        let _first = attach(&harness.route, 1);
        hello(&harness.route, 1, 100);
        let _second = attach(&harness.route, 2);
        hello(&harness.route, 2, 101);
        assert_eq!(harness.route.state().pages, 2);

        harness.route.disconnect(trace(1));
        assert_eq!(harness.route.state().pages, 1);
        assert_eq!(
            harness.route.state().phase,
            LocalPresencePhase::Running,
            "one window closing is not the user leaving",
        );

        std::thread::sleep(Duration::from_millis(120));
        harness.route.tick();
        assert_eq!(*harness.exits.lock().expect("exit counter"), 0);

        // The second one is.
        harness.route.disconnect(trace(2));
        assert_eq!(harness.route.state().pages, 0);
        std::thread::sleep(Duration::from_millis(120));
        harness.route.tick();
        assert_eq!(*harness.exits.lock().expect("exit counter"), 1);
    }

    #[test]
    fn the_last_page_leaving_starts_a_countdown_that_a_returning_page_cancels() {
        let harness = harness(Duration::from_millis(200));
        let _inbound = attach(&harness.route, 1);
        hello(&harness.route, 1, 100);
        harness.route.disconnect(trace(1));

        assert!(matches!(
            harness.route.state().phase,
            LocalPresencePhase::CountingDown { .. }
        ));

        let _returned = attach(&harness.route, 2);
        hello(&harness.route, 2, 100);
        assert_eq!(harness.route.state().phase, LocalPresencePhase::Running);

        std::thread::sleep(Duration::from_millis(260));
        harness.route.tick();
        assert_eq!(
            *harness.exits.lock().expect("exit counter"),
            0,
            "a page is open; the expired countdown belonged to a previous one"
        );
    }

    #[test]
    fn the_countdown_expires_once_when_no_page_returns() {
        let harness = harness(Duration::from_millis(80));
        let _inbound = attach(&harness.route, 1);
        hello(&harness.route, 1, 100);
        harness.route.disconnect(trace(1));

        std::thread::sleep(Duration::from_millis(120));
        harness.route.tick();
        harness.route.tick();
        harness.route.tick();

        assert_eq!(*harness.exits.lock().expect("exit counter"), 1);
        assert_eq!(harness.route.state().phase, LocalPresencePhase::Exiting);
    }

    #[test]
    fn a_page_that_returns_at_the_deadline_keeps_the_program_alive() {
        let harness = harness(Duration::from_millis(120));
        let _inbound = attach(&harness.route, 1);
        hello(&harness.route, 1, 100);
        harness.route.disconnect(trace(1));

        // The tab comes back with the countdown all but expired.
        std::thread::sleep(Duration::from_millis(110));
        let _returned = attach(&harness.route, 2);
        hello(&harness.route, 2, 100);
        std::thread::sleep(Duration::from_millis(60));
        harness.route.tick();

        assert_eq!(*harness.exits.lock().expect("exit counter"), 0);
        assert_eq!(harness.route.state().pages, 1);
    }

    #[test]
    fn the_console_shows_the_countdown_once_a_second_and_not_once_a_tick() {
        #[derive(Default)]
        struct Recorder {
            lines: Mutex<Vec<String>>,
        }
        impl crate::LocalConsoleSink for std::sync::Arc<Recorder> {
            fn line(&self, text: &str) {
                self.lines.lock().expect("recorder").push(text.to_owned());
            }
        }

        let recorder = std::sync::Arc::new(Recorder::default());
        let console = std::sync::Arc::new(
            crate::LocalConsole::new(crate::LocalConsoleLocale::EnUs)
                .with_sink(std::sync::Arc::new(recorder.clone())),
        );
        let route = LocalPresenceRoute::new(|| {})
            .with_countdown(Duration::from_millis(2_500))
            .with_console(console);

        let (outbound, _inbound) = OutboundSender::unbounded_pair();
        assert!(route.connect(trace(1), outbound));
        route.receive_text(
            trace(1),
            &serde_json::to_string(&WsClientEnvelope::new(
                trace(9_001),
                LocalPresenceCommandV1::Hello { tab_id: trace(100) },
            ))
            .expect("hello frame"),
        );
        route.disconnect(trace(1));

        // The maintenance timer runs four times a second. The console must
        // not: a countdown that repeated itself every 250ms would push every
        // other line out of the window before the user could read it.
        for _ in 0..12 {
            route.tick();
            std::thread::sleep(Duration::from_millis(100));
        }

        let lines = recorder.lines.lock().expect("recorder").clone();
        let countdowns = lines
            .iter()
            .filter(|line| line.contains("Exiting in"))
            .count();
        assert!(
            lines.iter().any(|line| line.contains("A page is open")),
            "{lines:?}"
        );
        assert!(
            lines.iter().any(|line| line.contains("A page closed")),
            "{lines:?}"
        );
        assert!(
            (2..=4).contains(&countdowns),
            "expected one line per remaining second, saw {countdowns}: {lines:?}",
        );
    }

    #[test]
    fn a_withheld_hello_landing_after_the_exit_decision_is_not_registered() {
        // The withheld socket's registration deadline must OUTLIVE the
        // countdown: with the shared 60ms deadline the sweep would close the
        // silent socket at the same tick that commits Exiting, and the test
        // would pass against the very race it exists to catch.
        let harness = harness_with(Duration::from_millis(40), Duration::from_secs(30));
        let _first = attach(&harness.route, 1);
        hello(&harness.route, 1, 100);
        harness.route.disconnect(trace(1));
        assert!(matches!(
            harness.route.state().phase,
            LocalPresencePhase::CountingDown { .. }
        ));

        // Admitted while the countdown is still running -- legal -- but the
        // HELLO is withheld past the expiry decision.
        let mut withheld = attach(&harness.route, 2);
        std::thread::sleep(Duration::from_millis(80));
        harness.route.tick();
        assert_eq!(*harness.exits.lock().expect("exit counter"), 1);
        assert_eq!(harness.route.state().phase, LocalPresencePhase::Exiting);

        // The HELLO lands after the decision. Registering it would tell the
        // page it is counted and then take the runtime away from it.
        hello(&harness.route, 2, 200);
        assert_eq!(harness.route.state().pages, 0);
        assert_eq!(harness.route.state().phase, LocalPresencePhase::Exiting);
        assert_eq!(
            *harness.exits.lock().expect("exit counter"),
            1,
            "the rejected HELLO neither cancels nor re-requests the exit",
        );
        assert!(
            withheld.try_recv().is_err(),
            "a HELLO after the exit decision is not answered REGISTERED",
        );
    }

    #[test]
    fn a_withheld_hello_after_the_exit_decision_logs_no_page_opened() {
        #[derive(Default)]
        struct Recorder {
            lines: Mutex<Vec<String>>,
        }
        impl crate::LocalConsoleSink for std::sync::Arc<Recorder> {
            fn line(&self, text: &str) {
                self.lines.lock().expect("recorder").push(text.to_owned());
            }
        }

        let recorder = std::sync::Arc::new(Recorder::default());
        let console = std::sync::Arc::new(
            crate::LocalConsole::new(crate::LocalConsoleLocale::EnUs)
                .with_sink(std::sync::Arc::new(recorder.clone())),
        );
        let route = LocalPresenceRoute::new(|| {})
            .with_countdown(Duration::from_millis(40))
            .with_hello_deadline(Duration::from_secs(30))
            .with_console(console);

        let (outbound, _inbound) = mpsc::unbounded_channel();
        assert!(route.connect(trace(1), outbound));
        hello(&route, 1, 100);
        route.disconnect(trace(1));

        let (outbound, _withheld) = mpsc::unbounded_channel();
        assert!(route.connect(trace(2), outbound));
        std::thread::sleep(Duration::from_millis(80));
        route.tick();
        assert_eq!(route.state().phase, LocalPresencePhase::Exiting);

        hello(&route, 2, 200);

        // The user must never read "a page is open" about a page the runtime
        // is in the middle of taking away.
        let lines = recorder.lines.lock().expect("recorder").clone();
        let opened = lines.iter().filter(|line| line.contains("A page is open")).count();
        let closed = lines.iter().filter(|line| line.contains("A page closed")).count();
        assert_eq!(opened, 1, "only the original page ever opened: {lines:?}");
        assert_eq!(closed, 1, "the rejected socket was never a page: {lines:?}");
    }

    #[test]
    fn a_sealed_route_neither_counts_nor_exits() {
        let harness = harness(Duration::from_millis(40));
        let _inbound = attach(&harness.route, 1);
        hello(&harness.route, 1, 100);
        harness.route.seal();

        let (outbound, _rejected) = OutboundSender::unbounded_pair();
        assert!(!harness.route.connect(trace(2), outbound));
        std::thread::sleep(Duration::from_millis(60));
        harness.route.tick();
        assert_eq!(*harness.exits.lock().expect("exit counter"), 0);
    }
}
