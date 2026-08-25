//! What the Windows console says while the program is running.
//!
//! The local build is a console executable the user starts themselves, so the
//! window it opens is a product surface: it is the only place someone can see
//! that the program started, that a page is attached, that a section opened,
//! that the safety gate is holding a suspect snapshot, and that the program is
//! about to exit. It is written for a person, in their language.
//!
//! Two things it deliberately is not. It is not a log of everything: nothing
//! on a polling or per-tick path may write here, or the one line that matters
//! scrolls away between two ticks of a four-times-a-second timer. And it never
//! prints a session nonce, a browser URL carrying one, a header, or a request
//! body -- a console is a place people paste screenshots from.

use std::sync::Arc;

use bcsp_local_user_state::LocaleOverride;
use time::OffsetDateTime;

/// The two languages the product ships, and the only two this prints in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalConsoleLocale {
    EnUs,
    ZhCn,
}

impl LocalConsoleLocale {
    /// The language the console speaks.
    ///
    /// The user's own setting wins, exactly as it does in the page. `System`
    /// falls back to the environment the process was started in, and anything
    /// unrecognised falls back to en-US -- the same precedence and the same
    /// fallback the browser side uses.
    pub fn resolve(setting: LocaleOverride, environment: Option<&str>) -> Self {
        match setting {
            LocaleOverride::EnUs => Self::EnUs,
            LocaleOverride::ZhCn => Self::ZhCn,
            LocaleOverride::System => match environment {
                Some(tag) => Self::from_tag(tag),
                None => Self::EnUs,
            },
        }
    }

    fn from_tag(tag: &str) -> Self {
        let normalized = tag.trim().replace('_', "-").to_ascii_lowercase();
        if normalized == "zh" || normalized.starts_with("zh-") {
            Self::ZhCn
        } else {
            Self::EnUs
        }
    }
}

/// Where a console line goes. Injected so a test can read what a user sees.
pub trait LocalConsoleSink: Send + Sync + 'static {
    fn line(&self, text: &str);
}

struct StdoutSink;

impl LocalConsoleSink for StdoutSink {
    fn line(&self, text: &str) {
        // `println!` rather than tracing: this is the product's own console
        // voice, and it must not be reformatted, filtered, or JSON-wrapped by
        // a subscriber configured for diagnostics.
        println!("{text}");
    }
}

/// Every event this console reports. One enum so the set is reviewable in one
/// place, and so adding a line means deciding what it says in both languages.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocalConsoleEvent {
    /// The runtime is up and serving on `origin` (never the browser URL: that
    /// one carries the session nonce).
    Started { origin: String },
    /// A page attached to, or left, the presence channel.
    PageOpened { pages: u64 },
    PageClosed { pages: u64 },
    /// The last page left; the program will exit unless one returns.
    ExitCountdown { seconds: u64 },
    /// A page came back inside the window.
    ExitCancelled { pages: u64 },
    /// The program is shutting down, and why.
    Exiting { reason: LocalExitReason },
    Stopped,
    /// A page connected to the alert channel, or left it.
    AlertsAttached,
    AlertsDetached,
    /// The runtime started or stopped actually watching a section.
    WatchArmed { section: String },
    WatchDisarmed { section: String },
    /// A watched section is open right now.
    SectionOpen { section: String },
    /// The snapshot-integrity gate withheld a suspect snapshot, or released.
    GateHolding { target: String },
    GateReleased { target: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalExitReason {
    /// Every page closed and none came back.
    NoPages,
    /// The page asked the runtime to exit.
    Requested,
    /// The console or the operating system asked.
    Signal,
}

/// The console itself.
pub struct LocalConsole {
    locale: LocalConsoleLocale,
    sink: Arc<dyn LocalConsoleSink>,
}

impl LocalConsole {
    pub fn new(locale: LocalConsoleLocale) -> Self {
        Self {
            locale,
            sink: Arc::new(StdoutSink),
        }
    }

    #[must_use]
    pub fn with_sink(mut self, sink: Arc<dyn LocalConsoleSink>) -> Self {
        self.sink = sink;
        self
    }

    pub const fn locale(&self) -> LocalConsoleLocale {
        self.locale
    }

    pub fn report(&self, event: &LocalConsoleEvent) {
        self.sink.line(&format!("{}  {}", stamp(), self.render(event)));
    }

    fn render(&self, event: &LocalConsoleEvent) -> String {
        match (self.locale, event) {
            (LocalConsoleLocale::EnUs, LocalConsoleEvent::Started { origin }) => {
                format!("RBCSP is running at {origin}")
            }
            (LocalConsoleLocale::ZhCn, LocalConsoleEvent::Started { origin }) => {
                format!("RBCSP 已启动，地址 {origin}")
            }
            (LocalConsoleLocale::EnUs, LocalConsoleEvent::PageOpened { pages }) => {
                format!("A page is open ({pages} open)")
            }
            (LocalConsoleLocale::ZhCn, LocalConsoleEvent::PageOpened { pages }) => {
                format!("有页面打开（当前 {pages} 个）")
            }
            (LocalConsoleLocale::EnUs, LocalConsoleEvent::PageClosed { pages }) => {
                format!("A page closed ({pages} open)")
            }
            (LocalConsoleLocale::ZhCn, LocalConsoleEvent::PageClosed { pages }) => {
                format!("有页面关闭（当前 {pages} 个）")
            }
            (LocalConsoleLocale::EnUs, LocalConsoleEvent::ExitCountdown { seconds }) => {
                format!("Every page has closed. Exiting in {seconds} seconds; reopen a page to cancel.")
            }
            (LocalConsoleLocale::ZhCn, LocalConsoleEvent::ExitCountdown { seconds }) => {
                format!("页面已全部关闭，{seconds} 秒后退出；页面回归即取消。")
            }
            (LocalConsoleLocale::EnUs, LocalConsoleEvent::ExitCancelled { pages }) => {
                format!("A page returned ({pages} open). The exit is cancelled.")
            }
            (LocalConsoleLocale::ZhCn, LocalConsoleEvent::ExitCancelled { pages }) => {
                format!("页面已回归（当前 {pages} 个），退出已取消。")
            }
            (LocalConsoleLocale::EnUs, LocalConsoleEvent::Exiting { reason }) => {
                format!("Shutting down: {}", match reason {
                    LocalExitReason::NoPages => "no page returned",
                    LocalExitReason::Requested => "the page asked RBCSP to exit",
                    LocalExitReason::Signal => "the console asked RBCSP to stop",
                })
            }
            (LocalConsoleLocale::ZhCn, LocalConsoleEvent::Exiting { reason }) => {
                format!("正在关闭：{}", match reason {
                    LocalExitReason::NoPages => "没有页面回归",
                    LocalExitReason::Requested => "页面请求退出",
                    LocalExitReason::Signal => "控制台请求停止",
                })
            }
            (LocalConsoleLocale::EnUs, LocalConsoleEvent::Stopped) => {
                "RBCSP has stopped. Your saved watches are kept for next time.".to_owned()
            }
            (LocalConsoleLocale::ZhCn, LocalConsoleEvent::Stopped) => {
                "RBCSP 已停止。已保存的监控意图会保留到下次启动。".to_owned()
            }
            (LocalConsoleLocale::EnUs, LocalConsoleEvent::AlertsAttached) => {
                "A page is listening for alerts".to_owned()
            }
            (LocalConsoleLocale::ZhCn, LocalConsoleEvent::AlertsAttached) => {
                "有页面正在接收提醒".to_owned()
            }
            (LocalConsoleLocale::EnUs, LocalConsoleEvent::AlertsDetached) => {
                "No page is listening for alerts".to_owned()
            }
            (LocalConsoleLocale::ZhCn, LocalConsoleEvent::AlertsDetached) => {
                "没有页面在接收提醒".to_owned()
            }
            (LocalConsoleLocale::EnUs, LocalConsoleEvent::WatchArmed { section }) => {
                format!("Watching {section}")
            }
            (LocalConsoleLocale::ZhCn, LocalConsoleEvent::WatchArmed { section }) => {
                format!("正在监控 {section}")
            }
            (LocalConsoleLocale::EnUs, LocalConsoleEvent::WatchDisarmed { section }) => {
                format!("Stopped watching {section}")
            }
            (LocalConsoleLocale::ZhCn, LocalConsoleEvent::WatchDisarmed { section }) => {
                format!("已停止监控 {section}")
            }
            (LocalConsoleLocale::EnUs, LocalConsoleEvent::SectionOpen { section }) => {
                format!("OPEN: {section}")
            }
            (LocalConsoleLocale::ZhCn, LocalConsoleEvent::SectionOpen { section }) => {
                format!("开放：{section}")
            }
            (LocalConsoleLocale::EnUs, LocalConsoleEvent::GateHolding { target }) => {
                format!("Safety gate is holding a suspect {target} snapshot; the last good data is still in use")
            }
            (LocalConsoleLocale::ZhCn, LocalConsoleEvent::GateHolding { target }) => {
                format!("安全门扣住了 {target} 的可疑快照；仍在使用上一份可信数据")
            }
            (LocalConsoleLocale::EnUs, LocalConsoleEvent::GateReleased { target }) => {
                format!("Safety gate released {target}; live data is in use again")
            }
            (LocalConsoleLocale::ZhCn, LocalConsoleEvent::GateReleased { target }) => {
                format!("安全门已放行 {target}；恢复使用实时数据")
            }
        }
    }
}

/// The time on each line.
///
/// UTC, and SAID to be UTC. Reading the local zone needs a platform offset
/// lookup this build does not take, and a bare "01:23:45" that is silently an
/// hour or twelve out is worse than a labelled one: someone comparing the
/// console against when they saw a course open would draw the wrong
/// conclusion and have no way to notice.
/// Turns two events raised deep in the open pipeline into console lines.
///
/// The snapshot-integrity gate lives in a crate that knows nothing about
/// consoles, locales, or which target it is running inside, and it should stay
/// that way. What it does have is a stable event code, so the console listens
/// for exactly those codes and says the rest itself. Anything else on the
/// diagnostic stream passes straight through untouched.
pub struct LocalConsoleLayer {
    console: Arc<LocalConsole>,
}

impl LocalConsoleLayer {
    pub const fn new(console: Arc<LocalConsole>) -> Self {
        Self { console }
    }
}

#[derive(Default)]
struct EventFields {
    code: Option<String>,
    target: Option<String>,
    section: Option<String>,
}

impl tracing::field::Visit for EventFields {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            "code" => self.code = Some(value.to_owned()),
            "target" => self.target = Some(value.to_owned()),
            "section" => self.section = Some(value.to_owned()),
            _ => {}
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        // `%value` reaches `record_str`; `?value` and the `Display` wrapper
        // both land here, and the gate names its target with one of those.
        let rendered = format!("{value:?}");
        let rendered = rendered.trim_matches('"').to_owned();
        match field.name() {
            "code" => self.code = Some(rendered),
            "target" => self.target = Some(rendered),
            "section" => self.section = Some(rendered),
            _ => {}
        }
    }
}

impl<S> tracing_subscriber::Layer<S> for LocalConsoleLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _context: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut fields = EventFields::default();
        event.record(&mut fields);
        let Some(code) = fields.code else { return };
        let target = || fields.target.clone().unwrap_or_else(|| "the course feed".to_owned());
        let section = || fields.section.clone().unwrap_or_else(|| "a section".to_owned());
        match code.as_str() {
            "OPEN_SNAPSHOT_GATE_HOLD" => {
                self.console.report(&LocalConsoleEvent::GateHolding { target: target() });
            }
            "OPEN_SNAPSHOT_GATE_RELEASED" => {
                self.console.report(&LocalConsoleEvent::GateReleased { target: target() });
            }
            "LOCAL_WATCH_ARMED" => {
                self.console.report(&LocalConsoleEvent::WatchArmed { section: section() });
            }
            "LOCAL_WATCH_DISARMED" => {
                self.console.report(&LocalConsoleEvent::WatchDisarmed { section: section() });
            }
            "LOCAL_SECTION_OPEN" => {
                self.console.report(&LocalConsoleEvent::SectionOpen { section: section() });
            }
            _ => {}
        }
    }
}

fn stamp() -> String {
    let now = OffsetDateTime::now_utc();
    format!("{:02}:{:02}:{:02}Z", now.hour(), now.minute(), now.second())
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct Recorder {
        lines: Mutex<Vec<String>>,
    }

    impl LocalConsoleSink for Arc<Recorder> {
        fn line(&self, text: &str) {
            self.lines.lock().expect("recorder").push(text.to_owned());
        }
    }

    fn console(locale: LocalConsoleLocale) -> (LocalConsole, Arc<Recorder>) {
        let recorder = Arc::new(Recorder::default());
        let console = LocalConsole::new(locale).with_sink(Arc::new(recorder.clone()));
        (console, recorder)
    }

    fn lines(recorder: &Recorder) -> Vec<String> {
        recorder.lines.lock().expect("recorder").clone()
    }

    #[test]
    fn the_users_own_setting_decides_the_language() {
        assert_eq!(
            LocalConsoleLocale::resolve(LocaleOverride::ZhCn, Some("en-US")),
            LocalConsoleLocale::ZhCn,
        );
        assert_eq!(
            LocalConsoleLocale::resolve(LocaleOverride::EnUs, Some("zh-CN")),
            LocalConsoleLocale::EnUs,
        );
    }

    #[test]
    fn system_follows_the_environment_and_falls_back_to_english() {
        for tag in ["zh", "zh-CN", "zh_CN.UTF-8", "ZH-cn"] {
            assert_eq!(
                LocalConsoleLocale::resolve(LocaleOverride::System, Some(tag)),
                LocalConsoleLocale::ZhCn,
                "{tag}",
            );
        }
        for tag in ["en-US", "fr-FR", "", "C"] {
            assert_eq!(
                LocalConsoleLocale::resolve(LocaleOverride::System, Some(tag)),
                LocalConsoleLocale::EnUs,
                "{tag}",
            );
        }
        assert_eq!(
            LocalConsoleLocale::resolve(LocaleOverride::System, None),
            LocalConsoleLocale::EnUs,
        );
    }

    #[test]
    fn every_event_says_something_different_in_each_language() {
        let events = [
            LocalConsoleEvent::Started {
                origin: "http://127.0.0.1:1234".to_owned(),
            },
            LocalConsoleEvent::PageOpened { pages: 1 },
            LocalConsoleEvent::PageClosed { pages: 0 },
            LocalConsoleEvent::ExitCountdown { seconds: 60 },
            LocalConsoleEvent::ExitCancelled { pages: 1 },
            LocalConsoleEvent::Exiting {
                reason: LocalExitReason::NoPages,
            },
            LocalConsoleEvent::Stopped,
            LocalConsoleEvent::AlertsAttached,
            LocalConsoleEvent::AlertsDetached,
            LocalConsoleEvent::WatchArmed {
                section: "92026 NB 12345".to_owned(),
            },
            LocalConsoleEvent::WatchDisarmed {
                section: "92026 NB 12345".to_owned(),
            },
            LocalConsoleEvent::SectionOpen {
                section: "92026 NB 12345".to_owned(),
            },
            LocalConsoleEvent::GateHolding {
                target: "92026 NB".to_owned(),
            },
            LocalConsoleEvent::GateReleased {
                target: "92026 NB".to_owned(),
            },
        ];
        let (english, english_lines) = console(LocalConsoleLocale::EnUs);
        let (chinese, chinese_lines) = console(LocalConsoleLocale::ZhCn);
        for event in &events {
            english.report(event);
            chinese.report(event);
        }
        let english_lines = lines(&english_lines);
        let chinese_lines = lines(&chinese_lines);
        assert_eq!(english_lines.len(), events.len());
        assert_eq!(chinese_lines.len(), events.len());
        for (index, (left, right)) in english_lines.iter().zip(&chinese_lines).enumerate() {
            // Every user-facing line follows the locale. A line that read the
            // same in both would be one that was never translated.
            assert_ne!(left, right, "event {index} reads the same in both languages");
        }
    }

    #[test]
    fn the_codes_other_crates_raise_become_lines_the_user_can_read() {
        use tracing_subscriber::layer::SubscriberExt;

        // The gate lives in a crate that knows nothing about consoles or
        // languages. It states a code; this is where that becomes a sentence.
        let recorder = Arc::new(Recorder::default());
        let console = Arc::new(
            LocalConsole::new(LocalConsoleLocale::ZhCn).with_sink(Arc::new(recorder.clone())),
        );
        let subscriber =
            tracing_subscriber::registry().with(LocalConsoleLayer::new(console.clone()));
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(code = "OPEN_SNAPSHOT_GATE_HOLD", target = "92026 NB");
            tracing::info!(code = "OPEN_SNAPSHOT_GATE_RELEASED", target = "92026 NB");
            tracing::info!(code = "LOCAL_WATCH_ARMED", section = "92026 NB 12345");
            tracing::info!(code = "LOCAL_WATCH_DISARMED", section = "92026 NB 12345");
            tracing::info!(code = "LOCAL_SECTION_OPEN", section = "92026 NB 12345");
            // Everything else on the diagnostic stream is left alone.
            tracing::info!(code = "SOMETHING_ELSE", detail = "ignored");
            tracing::warn!("a bare diagnostic with no code");
        });

        let reported = lines(&recorder);
        assert_eq!(reported.len(), 5, "{reported:?}");
        assert!(reported[0].contains("安全门"), "{}", reported[0]);
        assert!(reported[1].contains("放行"), "{}", reported[1]);
        assert!(reported[2].contains("正在监控 92026 NB 12345"), "{}", reported[2]);
        assert!(reported[3].contains("已停止监控"), "{}", reported[3]);
        assert!(reported[4].contains("开放：92026 NB 12345"), "{}", reported[4]);
    }

    #[test]
    fn the_countdown_reads_as_a_countdown() {
        let (console, recorder) = console(LocalConsoleLocale::EnUs);
        for seconds in [60, 30, 5, 0] {
            console.report(&LocalConsoleEvent::ExitCountdown { seconds });
        }
        let reported = lines(&recorder);
        assert!(reported[0].contains("60 seconds"), "{}", reported[0]);
        assert!(reported[3].contains("0 seconds"), "{}", reported[3]);
    }

    #[test]
    fn nothing_the_console_prints_carries_a_session() {
        // The browser URL carries the session nonce, so the console is given
        // the ORIGIN. This pins the distinction rather than trusting it.
        let (console, recorder) = console(LocalConsoleLocale::EnUs);
        console.report(&LocalConsoleEvent::Started {
            origin: "http://127.0.0.1:1234".to_owned(),
        });
        let reported = lines(&recorder);
        assert!(!reported[0].contains("session"), "{}", reported[0]);
        assert!(!reported[0].contains('?'), "{}", reported[0]);
    }
}
