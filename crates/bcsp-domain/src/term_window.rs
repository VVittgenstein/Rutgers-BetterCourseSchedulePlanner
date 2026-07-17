use bcsp_contracts::TermId;
use jiff::{Timestamp, civil::Date, tz::TimeZone};
use thiserror::Error;
use time::OffsetDateTime;

pub const RUTGERS_TERM_CALENDAR_VERSION: &str = "2026-07-17.2";
pub const RUTGERS_TERM_CALENDAR_DIAGNOSTIC_CODE: &str = "TERM_CALENDAR_UNAVAILABLE";
pub const RUTGERS_TIME_ZONE: &str = "America/New_York";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RutgersTermWindowScope {
    Public,
    Local,
}

impl RutgersTermWindowScope {
    const fn offset_bounds(self) -> (i8, i8) {
        match self {
            Self::Public => (0, 1),
            Self::Local => (-2, 2),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RutgersTermSeason {
    Winter,
    Spring,
    Summer,
    Fall,
}

impl RutgersTermSeason {
    pub const fn term_code(self) -> u8 {
        match self {
            Self::Winter => 0,
            Self::Spring => 1,
            Self::Summer => 7,
            Self::Fall => 9,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RutgersCalendarSource {
    pub academic_year: &'static str,
    pub fall_spring_source_url: &'static str,
    pub winter_summer_source_url: &'static str,
    /// Whether the linked official calendar published exact Winter and Summer
    /// session boundaries for this academic year when this table was retrieved.
    ///
    /// `false` is deliberately not filled from a calendar formula. A term
    /// window that needs one of those unpublished sessions is unavailable.
    pub winter_summer_dates_verified: bool,
    pub retrieved_on: &'static str,
}

const DEFAULT_CALENDAR_SOURCE_URL: &str =
    "https://senate.rutgers.edu/wp-content/uploads/2019/10/ASRAC-Report-S-1509-September-2017.pdf";
const PUBLISHED_SESSION_CALENDAR_URL: &str = "https://scheduling.rutgers.edu/academic-calendar/";
const ACADEMIC_CALENDAR_DIRECTORY_URL: &str =
    "https://academicaffairs.rutgers.edu/academic-calendar-directory/";

pub const RUTGERS_CALENDAR_SOURCES: &[RutgersCalendarSource] = &[
    RutgersCalendarSource {
        academic_year: "2025-2026",
        fall_spring_source_url: DEFAULT_CALENDAR_SOURCE_URL,
        winter_summer_source_url: PUBLISHED_SESSION_CALENDAR_URL,
        winter_summer_dates_verified: true,
        retrieved_on: "2026-07-17",
    },
    RutgersCalendarSource {
        academic_year: "2026-2027",
        fall_spring_source_url: DEFAULT_CALENDAR_SOURCE_URL,
        winter_summer_source_url: PUBLISHED_SESSION_CALENDAR_URL,
        winter_summer_dates_verified: true,
        retrieved_on: "2026-07-17",
    },
    RutgersCalendarSource {
        academic_year: "2027-2028",
        fall_spring_source_url: DEFAULT_CALENDAR_SOURCE_URL,
        winter_summer_source_url: PUBLISHED_SESSION_CALENDAR_URL,
        winter_summer_dates_verified: false,
        retrieved_on: "2026-07-17",
    },
    RutgersCalendarSource {
        academic_year: "2028-2029",
        fall_spring_source_url: DEFAULT_CALENDAR_SOURCE_URL,
        winter_summer_source_url: ACADEMIC_CALENDAR_DIRECTORY_URL,
        winter_summer_dates_verified: false,
        retrieved_on: "2026-07-17",
    },
    RutgersCalendarSource {
        academic_year: "2029-2030",
        fall_spring_source_url: DEFAULT_CALENDAR_SOURCE_URL,
        winter_summer_source_url: ACADEMIC_CALENDAR_DIRECTORY_URL,
        winter_summer_dates_verified: false,
        retrieved_on: "2026-07-17",
    },
    RutgersCalendarSource {
        academic_year: "2030-2031",
        fall_spring_source_url: DEFAULT_CALENDAR_SOURCE_URL,
        winter_summer_source_url: ACADEMIC_CALENDAR_DIRECTORY_URL,
        winter_summer_dates_verified: false,
        retrieved_on: "2026-07-17",
    },
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RutgersVisibleTerm {
    term: TermId,
    season: RutgersTermSeason,
    year: i16,
    relative_offset: i8,
    starts_on: Date,
    ends_on: Date,
}

impl RutgersVisibleTerm {
    pub const fn term(&self) -> &TermId {
        &self.term
    }

    pub const fn season(&self) -> RutgersTermSeason {
        self.season
    }

    pub const fn year(&self) -> i16 {
        self.year
    }

    pub const fn relative_offset(&self) -> i8 {
        self.relative_offset
    }

    pub const fn starts_on(&self) -> Date {
        self.starts_on
    }

    pub const fn ends_on(&self) -> Date {
        self.ends_on
    }

    pub const fn watchable(&self) -> bool {
        matches!(self.relative_offset, 0 | 1)
    }

    pub const fn auto_managed(&self) -> bool {
        matches!(self.relative_offset, 0 | 1)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RutgersTermWindow {
    rutgers_day: Date,
    current_term: TermId,
    next_term: TermId,
    visible_terms: Vec<RutgersVisibleTerm>,
}

impl RutgersTermWindow {
    pub fn at(
        now: OffsetDateTime,
        scope: RutgersTermWindowScope,
    ) -> Result<Self, RutgersTermWindowError> {
        let rutgers_day = rutgers_day_at(now)?;
        let current_term = current_term_entry(rutgers_day)
            .ok_or(RutgersTermWindowError::CalendarOutOfRange { rutgers_day })?;
        let (minimum_offset, maximum_offset) = scope.offset_bounds();
        let next_term = current_term
            .identity()
            .offset(1)
            .and_then(term_calendar_entry)
            .ok_or(RutgersTermWindowError::CalendarOutOfRange { rutgers_day })?;

        let visible_terms = (minimum_offset..=maximum_offset)
            .map(|relative_offset| {
                current_term
                    .identity()
                    .offset(relative_offset)
                    .and_then(term_calendar_entry)
                    .ok_or(RutgersTermWindowError::CalendarOutOfRange { rutgers_day })?
                    .visible_term(relative_offset)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            rutgers_day,
            current_term: current_term.term_id()?,
            next_term: next_term.term_id()?,
            visible_terms,
        })
    }

    pub const fn rutgers_day(&self) -> Date {
        self.rutgers_day
    }

    pub const fn current_term(&self) -> &TermId {
        &self.current_term
    }

    pub const fn next_term(&self) -> &TermId {
        &self.next_term
    }

    pub fn visible_terms(&self) -> &[RutgersVisibleTerm] {
        &self.visible_terms
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RutgersTermWindowError {
    #[error("the America/New_York time zone is unavailable")]
    TimeZoneUnavailable,
    #[error("the timestamp cannot be represented in America/New_York")]
    TimestampOutOfRange,
    #[error("the bundled Rutgers term calendar does not cover {rutgers_day}")]
    CalendarOutOfRange { rutgers_day: Date },
    #[error("the bundled Rutgers term identity is invalid")]
    InvalidTermIdentity,
}

impl RutgersTermWindowError {
    pub const fn diagnostic_code(self) -> &'static str {
        RUTGERS_TERM_CALENDAR_DIAGNOSTIC_CODE
    }
}

#[derive(Clone, Copy)]
struct TermCalendarEntry {
    season: RutgersTermSeason,
    year: i16,
    starts_on: Date,
    ends_on: Date,
}

impl TermCalendarEntry {
    const fn identity(self) -> TermIdentity {
        TermIdentity {
            season: self.season,
            year: self.year,
        }
    }

    fn term_id(self) -> Result<TermId, RutgersTermWindowError> {
        TermId::try_from(format!("{}{}", self.season.term_code(), self.year))
            .map_err(|_| RutgersTermWindowError::InvalidTermIdentity)
    }

    fn visible_term(
        self,
        relative_offset: i8,
    ) -> Result<RutgersVisibleTerm, RutgersTermWindowError> {
        Ok(RutgersVisibleTerm {
            term: self.term_id()?,
            season: self.season,
            year: self.year,
            relative_offset,
            starts_on: self.starts_on,
            ends_on: self.ends_on,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TermIdentity {
    season: RutgersTermSeason,
    year: i16,
}

impl TermIdentity {
    fn next(self) -> Option<Self> {
        match self.season {
            RutgersTermSeason::Winter => Some(Self {
                season: RutgersTermSeason::Spring,
                year: self.year,
            }),
            RutgersTermSeason::Spring => Some(Self {
                season: RutgersTermSeason::Summer,
                year: self.year,
            }),
            RutgersTermSeason::Summer => Some(Self {
                season: RutgersTermSeason::Fall,
                year: self.year,
            }),
            RutgersTermSeason::Fall => self.year.checked_add(1).map(|year| Self {
                season: RutgersTermSeason::Winter,
                year,
            }),
        }
    }

    fn previous(self) -> Option<Self> {
        match self.season {
            RutgersTermSeason::Winter => self.year.checked_sub(1).map(|year| Self {
                season: RutgersTermSeason::Fall,
                year,
            }),
            RutgersTermSeason::Spring => Some(Self {
                season: RutgersTermSeason::Winter,
                year: self.year,
            }),
            RutgersTermSeason::Summer => Some(Self {
                season: RutgersTermSeason::Spring,
                year: self.year,
            }),
            RutgersTermSeason::Fall => Some(Self {
                season: RutgersTermSeason::Summer,
                year: self.year,
            }),
        }
    }

    fn offset(self, offset: i8) -> Option<Self> {
        let mut identity = self;
        if offset.is_negative() {
            for _ in 0..offset.unsigned_abs() {
                identity = identity.previous()?;
            }
        } else {
            for _ in 0..u8::try_from(offset).ok()? {
                identity = identity.next()?;
            }
        }
        Some(identity)
    }
}

#[derive(Clone, Copy)]
struct TermDateRange {
    starts_on: Date,
    ends_on: Date,
}

#[derive(Clone, Copy)]
struct AcademicYearCalendarEntry {
    fall_year: i16,
    fall: TermDateRange,
    winter: Option<TermDateRange>,
    spring: TermDateRange,
    summer: Option<TermDateRange>,
}

// Fall and Spring values are from the Senate's approved Default University
// Academic Calendar through 2030-31. The more current Rutgers University
// Academic Calendar confirms 2025-26 through 2027-28 and is the only source
// used here for exact Winter and Summer session boundaries. It currently
// publishes Winter through 2028 but Summer only through 2027. Missing session
// dates remain `None`; they are never inferred from recurring weekday rules.
const ACADEMIC_YEAR_CALENDAR: &[AcademicYearCalendarEntry] = &[
    AcademicYearCalendarEntry {
        fall_year: 2025,
        fall: TermDateRange {
            starts_on: Date::constant(2025, 9, 2),
            ends_on: Date::constant(2025, 12, 22),
        },
        winter: Some(TermDateRange {
            starts_on: Date::constant(2025, 12, 22),
            ends_on: Date::constant(2026, 1, 16),
        }),
        spring: TermDateRange {
            starts_on: Date::constant(2026, 1, 20),
            ends_on: Date::constant(2026, 5, 13),
        },
        summer: Some(TermDateRange {
            starts_on: Date::constant(2026, 5, 26),
            ends_on: Date::constant(2026, 8, 12),
        }),
    },
    AcademicYearCalendarEntry {
        fall_year: 2026,
        fall: TermDateRange {
            starts_on: Date::constant(2026, 9, 1),
            ends_on: Date::constant(2026, 12, 22),
        },
        winter: Some(TermDateRange {
            starts_on: Date::constant(2026, 12, 23),
            ends_on: Date::constant(2027, 1, 15),
        }),
        spring: TermDateRange {
            starts_on: Date::constant(2027, 1, 19),
            ends_on: Date::constant(2027, 5, 12),
        },
        summer: Some(TermDateRange {
            starts_on: Date::constant(2027, 6, 1),
            ends_on: Date::constant(2027, 8, 18),
        }),
    },
    AcademicYearCalendarEntry {
        fall_year: 2027,
        fall: TermDateRange {
            starts_on: Date::constant(2027, 9, 1),
            ends_on: Date::constant(2027, 12, 23),
        },
        winter: Some(TermDateRange {
            starts_on: Date::constant(2027, 12, 23),
            ends_on: Date::constant(2028, 1, 14),
        }),
        spring: TermDateRange {
            starts_on: Date::constant(2028, 1, 18),
            ends_on: Date::constant(2028, 5, 10),
        },
        summer: None,
    },
    AcademicYearCalendarEntry {
        fall_year: 2028,
        fall: TermDateRange {
            starts_on: Date::constant(2028, 9, 5),
            ends_on: Date::constant(2028, 12, 22),
        },
        winter: None,
        spring: TermDateRange {
            starts_on: Date::constant(2029, 1, 16),
            ends_on: Date::constant(2029, 5, 9),
        },
        summer: None,
    },
    AcademicYearCalendarEntry {
        fall_year: 2029,
        fall: TermDateRange {
            starts_on: Date::constant(2029, 9, 4),
            ends_on: Date::constant(2029, 12, 21),
        },
        winter: None,
        spring: TermDateRange {
            starts_on: Date::constant(2030, 1, 22),
            ends_on: Date::constant(2030, 5, 15),
        },
        summer: None,
    },
    AcademicYearCalendarEntry {
        fall_year: 2030,
        fall: TermDateRange {
            starts_on: Date::constant(2030, 9, 3),
            ends_on: Date::constant(2030, 12, 23),
        },
        winter: None,
        spring: TermDateRange {
            starts_on: Date::constant(2031, 1, 21),
            ends_on: Date::constant(2031, 5, 14),
        },
        summer: None,
    },
];

fn rutgers_day_at(value: OffsetDateTime) -> Result<Date, RutgersTermWindowError> {
    let nanosecond = i32::try_from(value.nanosecond())
        .map_err(|_| RutgersTermWindowError::TimestampOutOfRange)?;
    let timestamp = Timestamp::new(value.unix_timestamp(), nanosecond)
        .map_err(|_| RutgersTermWindowError::TimestampOutOfRange)?;
    let time_zone = TimeZone::get(RUTGERS_TIME_ZONE)
        .map_err(|_| RutgersTermWindowError::TimeZoneUnavailable)?;
    Ok(timestamp.to_zoned(time_zone).date())
}

fn known_term_calendar() -> impl Iterator<Item = TermCalendarEntry> {
    ACADEMIC_YEAR_CALENDAR.iter().flat_map(|academic_year| {
        let session_year = academic_year.fall_year + 1;
        [
            Some(term_calendar_entry_from_range(
                RutgersTermSeason::Fall,
                academic_year.fall_year,
                academic_year.fall,
            )),
            academic_year.winter.map(|range| {
                term_calendar_entry_from_range(RutgersTermSeason::Winter, session_year, range)
            }),
            Some(term_calendar_entry_from_range(
                RutgersTermSeason::Spring,
                session_year,
                academic_year.spring,
            )),
            academic_year.summer.map(|range| {
                term_calendar_entry_from_range(RutgersTermSeason::Summer, session_year, range)
            }),
        ]
        .into_iter()
        .flatten()
    })
}

const fn term_calendar_entry_from_range(
    season: RutgersTermSeason,
    year: i16,
    range: TermDateRange,
) -> TermCalendarEntry {
    TermCalendarEntry {
        season,
        year,
        starts_on: range.starts_on,
        ends_on: range.ends_on,
    }
}

fn term_calendar_entry(identity: TermIdentity) -> Option<TermCalendarEntry> {
    known_term_calendar().find(|term| term.identity() == identity)
}

fn current_term_entry(day: Date) -> Option<TermCalendarEntry> {
    if let Some(term) = known_term_calendar()
        .filter(|term| term.starts_on <= day && day <= term.ends_on)
        .max_by_key(|term| term.starts_on)
    {
        return Some(term);
    }

    let previous = known_term_calendar()
        .filter(|term| term.ends_on < day)
        .max_by_key(|term| term.ends_on)?;
    let next = known_term_calendar()
        .filter(|term| day < term.starts_on)
        .min_by_key(|term| term.starts_on)?;

    (previous.identity().next() == Some(next.identity())).then_some(next)
}

#[cfg(test)]
mod tests {
    use time::{Date as TimeDate, Month, PrimitiveDateTime, Time, UtcOffset};

    use super::*;

    fn at_utc(year: i32, month: Month, day: u8, hour: u8) -> OffsetDateTime {
        PrimitiveDateTime::new(
            TimeDate::from_calendar_date(year, month, day).expect("test date"),
            Time::from_hms(hour, 0, 0).expect("test time"),
        )
        .assume_offset(UtcOffset::UTC)
    }

    #[test]
    fn july_17_2026_is_summer_and_fall_is_next() {
        let window = RutgersTermWindow::at(
            at_utc(2026, Month::July, 17, 12),
            RutgersTermWindowScope::Local,
        )
        .expect("covered calendar day");

        assert_eq!(window.current_term().as_str(), "72026");
        assert_eq!(window.next_term().as_str(), "92026");
        assert_eq!(window.visible_terms().len(), 5);
        assert_eq!(
            window
                .visible_terms()
                .iter()
                .map(|term| term.relative_offset())
                .collect::<Vec<_>>(),
            vec![-2, -1, 0, 1, 2]
        );
    }

    #[test]
    fn public_window_only_exposes_current_and_next() {
        let window = RutgersTermWindow::at(
            at_utc(2026, Month::July, 17, 12),
            RutgersTermWindowScope::Public,
        )
        .expect("covered calendar day");

        assert_eq!(
            window
                .visible_terms()
                .iter()
                .map(|term| (term.term().as_str(), term.relative_offset()))
                .collect::<Vec<_>>(),
            vec![("72026", 0), ("92026", 1)]
        );
        assert!(window.visible_terms().iter().all(|term| term.watchable()));
    }

    #[test]
    fn local_watchability_and_auto_management_only_cover_current_and_next() {
        let window = RutgersTermWindow::at(
            at_utc(2026, Month::July, 17, 12),
            RutgersTermWindowScope::Local,
        )
        .expect("covered calendar day");

        assert_eq!(
            window
                .visible_terms()
                .iter()
                .map(|term| (
                    term.relative_offset(),
                    term.watchable(),
                    term.auto_managed()
                ))
                .collect::<Vec<_>>(),
            vec![
                (-2, false, false),
                (-1, false, false),
                (0, true, true),
                (1, true, true),
                (2, false, false),
            ]
        );
    }

    #[test]
    fn intersession_days_roll_forward_to_the_next_term() {
        for (month, day, expected) in [
            (Month::January, 18, "12026"),
            (Month::May, 20, "72026"),
            (Month::August, 20, "92026"),
        ] {
            let window =
                RutgersTermWindow::at(at_utc(2026, month, day, 12), RutgersTermWindowScope::Public)
                    .expect("covered intersession");
            assert_eq!(window.current_term().as_str(), expected);
        }
    }

    #[test]
    fn later_starting_winter_wins_the_fall_exam_overlap() {
        let window = RutgersTermWindow::at(
            at_utc(2025, Month::December, 22, 17),
            RutgersTermWindowScope::Public,
        )
        .expect("covered overlap");
        assert_eq!(window.current_term().as_str(), "02026");
    }

    #[test]
    fn america_new_york_day_is_derived_from_the_instant() {
        let window = RutgersTermWindow::at(
            at_utc(2026, Month::July, 17, 2),
            RutgersTermWindowScope::Public,
        )
        .expect("covered calendar day");
        assert_eq!(window.rutgers_day(), Date::constant(2026, 7, 16));
    }

    #[test]
    fn local_five_term_window_is_complete_at_both_verified_boundaries() {
        for (year, month, day, expected) in [
            (
                2026,
                Month::February,
                1,
                vec!["92025", "02026", "12026", "72026", "92026"],
            ),
            (
                2027,
                Month::October,
                1,
                vec!["12027", "72027", "92027", "02028", "12028"],
            ),
        ] {
            let window =
                RutgersTermWindow::at(at_utc(year, month, day, 12), RutgersTermWindowScope::Local)
                    .expect("the complete five-term boundary is covered");

            assert_eq!(
                window
                    .visible_terms()
                    .iter()
                    .map(|term| term.term().as_str())
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }

    #[test]
    fn senate_fall_spring_dates_are_recorded_through_2030_31() {
        let fall_2030 = term_calendar_entry(TermIdentity {
            season: RutgersTermSeason::Fall,
            year: 2030,
        })
        .expect("the Senate publishes Fall 2030");
        let spring_2031 = term_calendar_entry(TermIdentity {
            season: RutgersTermSeason::Spring,
            year: 2031,
        })
        .expect("the Senate publishes Spring 2031");

        assert_eq!(fall_2030.starts_on, Date::constant(2030, 9, 3));
        assert_eq!(fall_2030.ends_on, Date::constant(2030, 12, 23));
        assert_eq!(spring_2031.starts_on, Date::constant(2031, 1, 21));
        assert_eq!(spring_2031.ends_on, Date::constant(2031, 5, 14));
        assert_eq!(
            RUTGERS_CALENDAR_SOURCES
                .last()
                .map(|source| source.academic_year),
            Some("2030-2031")
        );
    }

    #[test]
    fn unpublished_session_dates_are_not_inferred() {
        assert_eq!(
            RUTGERS_CALENDAR_SOURCES
                .iter()
                .map(|source| (source.academic_year, source.winter_summer_dates_verified))
                .collect::<Vec<_>>(),
            vec![
                ("2025-2026", true),
                ("2026-2027", true),
                ("2027-2028", false),
                ("2028-2029", false),
                ("2029-2030", false),
                ("2030-2031", false),
            ]
        );

        for (year, month, day) in [(2028, Month::April, 1), (2028, Month::September, 10)] {
            let error =
                RutgersTermWindow::at(at_utc(year, month, day, 12), RutgersTermWindowScope::Public)
                    .expect_err("the adjacent unpublished session makes the window unavailable");
            assert_eq!(
                error.diagnostic_code(),
                RUTGERS_TERM_CALENDAR_DIAGNOSTIC_CODE
            );
        }
    }

    #[test]
    fn an_uncovered_date_returns_the_stable_calendar_diagnostic() {
        let error = RutgersTermWindow::at(
            at_utc(2030, Month::January, 1, 12),
            RutgersTermWindowScope::Local,
        )
        .expect_err("2030 is outside the bundled table");

        assert!(matches!(
            error,
            RutgersTermWindowError::CalendarOutOfRange { .. }
        ));
        assert_eq!(
            error.diagnostic_code(),
            RUTGERS_TERM_CALENDAR_DIAGNOSTIC_CODE
        );
    }
}
