use std::time::Duration;

use time::OffsetDateTime;

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct WatchInstant(u64);

impl WatchInstant {
    pub const ZERO: Self = Self(0);

    pub const fn from_milliseconds(milliseconds: u64) -> Self {
        Self(milliseconds)
    }

    pub const fn as_milliseconds(self) -> u64 {
        self.0
    }

    pub fn saturating_add(self, duration: Duration) -> Self {
        let milliseconds = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
        Self(self.0.saturating_add(milliseconds))
    }

    pub const fn elapsed_since(self, earlier: Self) -> Duration {
        Duration::from_millis(self.0.saturating_sub(earlier.0))
    }
}

pub trait WatchClock {
    fn now(&mut self) -> WatchInstant;

    fn wall_now(&mut self) -> OffsetDateTime;
}
