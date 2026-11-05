//! When a rule runs: its occurrence on a given day and its next one, in Europe/Rome (Phase 3,
//! section 6). Pure functions of a date or of "now", so tests need no clock.

use chrono::{
    DateTime, Datelike, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, TimeZone, Utc,
    Weekday,
};
use chrono_tz::Tz;
use serde::Serialize;

use super::holidays;
use crate::time::TIME_ZONE;

/// How far `Schedule::next` looks ahead: a pause lasts at most a year, plus a few weeks.
const NEXT_HORIZON_DAYS: usize = 400;

/// The calendar of a rule: on which days, at what time, and what stops it.
#[derive(Debug, Clone, PartialEq)]
pub struct Schedule {
    /// Days of the week; ignored when `date` is set.
    pub days: Vec<Weekday>,
    /// A one-off rule runs on this date only.
    pub date: Option<NaiveDate>,
    /// Local time in Europe/Rome.
    pub time: NaiveTime,
    pub skip_holidays: bool,
    /// The last day skipped, included (decision AH).
    pub paused_until: Option<NaiveDate>,
    pub enabled: bool,
}

/// What a rule does on one day.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Day {
    /// It runs at `at`.
    Due { at: DateTime<Tz> },
    /// Not one of its days.
    NotScheduled,
    /// One of its days, but a public holiday it skips.
    Holiday { holiday: &'static str },
    /// Paused until `until`, included.
    Paused { until: NaiveDate },
    /// Turned off.
    Disabled,
}

impl Schedule {
    /// What the rule does on `date` (a calendar day in Europe/Rome, decision AI).
    pub fn on(&self, date: NaiveDate) -> Day {
        if !self.enabled {
            return Day::Disabled;
        }
        // A pause wins over "not one of its days", so a paused rule says so every day.
        if let Some(until) = self.paused_until.filter(|until| date <= *until) {
            return Day::Paused { until };
        }
        let scheduled = match self.date {
            Some(only) => date == only,
            None => self.days.contains(&date.weekday()),
        };
        if !scheduled {
            return Day::NotScheduled;
        }
        if self.skip_holidays
            && let Some(holiday) = holidays::holiday(date)
        {
            return Day::Holiday { holiday };
        }
        Day::Due {
            at: local_time(date, self.time),
        }
    }

    /// The first occurrence at or after `now`; `None` when the rule will not run again.
    pub fn next(&self, now: DateTime<Utc>) -> Option<DateTime<Tz>> {
        if !self.enabled {
            return None;
        }
        let now = now.with_timezone(&TIME_ZONE);
        let today = now.date_naive();
        today
            .iter_days()
            .take(NEXT_HORIZON_DAYS)
            .take_while(|date| self.date.is_none_or(|only| *date <= only))
            .find_map(|date| match self.on(date) {
                Day::Due { at } if at >= now => Some(at),
                _ => None,
            })
    }
}

/// Today's date in Europe/Rome: at 23:30 UTC it is already the next day there.
pub fn local_date(now: DateTime<Utc>) -> NaiveDate {
    now.with_timezone(&TIME_ZONE).date_naive()
}

/// `time` on `date` in Europe/Rome. A time that does not exist (the hour skipped when daylight
/// saving starts) moves to the next valid minute; a time that happens twice (the hour repeated
/// when it ends) is the first one.
pub fn local_time(date: NaiveDate, time: NaiveTime) -> DateTime<Tz> {
    let mut local: NaiveDateTime = date.and_time(time);
    // A gap lasts one hour in Europe/Rome; two are a safe bound.
    for _ in 0..=120 {
        match TIME_ZONE.from_local_datetime(&local) {
            LocalResult::Single(time) => return time,
            LocalResult::Ambiguous(first, _) => return first,
            LocalResult::None => local += TimeDelta::minutes(1),
        }
    }
    // Not reachable in Europe/Rome; read the time as UTC rather than fail.
    TIME_ZONE.from_utc_datetime(&date.and_time(time))
}

#[cfg(test)]
mod tests {
    use super::*;
    use Weekday::{Fri, Mon, Sat, Sun, Thu, Tue, Wed};

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    fn time(hour: u32, minute: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
    }

    /// A UTC instant, as the fake clock gives it.
    fn utc(text: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(text).unwrap().to_utc()
    }

    fn weekdays(hour: u32, minute: u32) -> Schedule {
        Schedule {
            days: vec![Mon, Tue, Wed, Thu, Fri],
            date: None,
            time: time(hour, minute),
            skip_holidays: true,
            paused_until: None,
            enabled: true,
        }
    }

    fn due(text: &str) -> Day {
        Day::Due {
            at: DateTime::parse_from_rfc3339(text)
                .unwrap()
                .with_timezone(&TIME_ZONE),
        }
    }

    fn next(schedule: &Schedule, now: &str) -> Option<String> {
        schedule.next(utc(now)).map(|at| at.to_rfc3339())
    }

    #[test]
    fn days_of_the_week() {
        let rule = weekdays(7, 40);
        // Thursday 5 and Saturday 7 November 2026.
        assert_eq!(rule.on(date(2026, 11, 5)), due("2026-11-05T07:40:00+01:00"));
        assert_eq!(rule.on(date(2026, 11, 7)), Day::NotScheduled);
        assert_eq!(rule.on(date(2026, 11, 8)), Day::NotScheduled);
    }

    #[test]
    fn holidays_are_skipped_when_asked() {
        let mut rule = weekdays(7, 40);
        // Easter Monday, San Giovanni (a Wednesday in 2026), Immacolata (a Tuesday).
        assert_eq!(
            rule.on(date(2026, 4, 6)),
            Day::Holiday {
                holiday: "Pasquetta"
            }
        );
        assert_eq!(
            rule.on(date(2026, 6, 24)),
            Day::Holiday {
                holiday: "San Giovanni"
            }
        );
        assert_eq!(
            rule.on(date(2026, 12, 8)),
            Day::Holiday {
                holiday: "Immacolata"
            }
        );
        // A holiday on a day the rule does not run is just not scheduled.
        rule.days = vec![Sat];
        assert_eq!(rule.on(date(2026, 4, 6)), Day::NotScheduled);

        let mut rule = weekdays(7, 40);
        rule.skip_holidays = false;
        assert_eq!(rule.on(date(2026, 4, 6)), due("2026-04-06T07:40:00+02:00"));
    }

    #[test]
    fn pause_includes_its_last_day() {
        let mut rule = weekdays(7, 40);
        // "Salta oggi" on Thursday 5 November: paused until that day.
        rule.paused_until = Some(date(2026, 11, 5));
        assert_eq!(
            rule.on(date(2026, 11, 5)),
            Day::Paused {
                until: date(2026, 11, 5)
            }
        );
        assert_eq!(rule.on(date(2026, 11, 6)), due("2026-11-06T07:40:00+01:00"));
        // A paused rule says so on its other days too.
        rule.paused_until = Some(date(2026, 11, 16));
        assert_eq!(
            rule.on(date(2026, 11, 8)),
            Day::Paused {
                until: date(2026, 11, 16)
            }
        );
        assert_eq!(
            next(&rule, "2026-11-05T06:00:00Z").as_deref(),
            Some("2026-11-17T07:40:00+01:00")
        );
    }

    #[test]
    fn disabled_rules_never_run() {
        let mut rule = weekdays(7, 40);
        rule.enabled = false;
        assert_eq!(rule.on(date(2026, 11, 5)), Day::Disabled);
        assert_eq!(next(&rule, "2026-11-05T06:00:00Z"), None);
    }

    #[test]
    fn one_off_dates() {
        let rule = Schedule {
            days: Vec::new(),
            date: Some(date(2026, 11, 5)),
            ..weekdays(8, 5)
        };
        assert_eq!(rule.on(date(2026, 11, 5)), due("2026-11-05T08:05:00+01:00"));
        assert_eq!(rule.on(date(2026, 11, 12)), Day::NotScheduled);
        assert_eq!(
            next(&rule, "2026-11-04T12:00:00Z").as_deref(),
            Some("2026-11-05T08:05:00+01:00")
        );
        // Once its time has passed, it will not run again.
        assert_eq!(next(&rule, "2026-11-05T07:06:00Z"), None);
    }

    #[test]
    fn next_occurrence() {
        let rule = weekdays(7, 40);
        // Thursday at 07:00 in Rome: today. At 07:40 exactly: still today. Later: tomorrow.
        assert_eq!(
            next(&rule, "2026-11-05T06:00:00Z").as_deref(),
            Some("2026-11-05T07:40:00+01:00")
        );
        assert_eq!(
            next(&rule, "2026-11-05T06:40:00Z").as_deref(),
            Some("2026-11-05T07:40:00+01:00")
        );
        assert_eq!(
            next(&rule, "2026-11-05T06:41:00Z").as_deref(),
            Some("2026-11-06T07:40:00+01:00")
        );
        // Friday evening: Monday. Before Christmas: past the holidays, on 28 December.
        assert_eq!(
            next(&rule, "2026-11-06T18:00:00Z").as_deref(),
            Some("2026-11-09T07:40:00+01:00")
        );
        assert_eq!(
            next(&rule, "2026-12-24T12:00:00Z").as_deref(),
            Some("2026-12-28T07:40:00+01:00")
        );
        // Across the new year: Friday 1 January 2027 is a holiday, so Monday 4.
        assert_eq!(
            next(&rule, "2026-12-31T12:00:00Z").as_deref(),
            Some("2027-01-04T07:40:00+01:00")
        );
    }

    #[test]
    fn midnight_is_local() {
        // 23:30 UTC on Thursday 5 November is 00:30 on Friday 6 in Rome.
        assert_eq!(local_date(utc("2026-11-05T23:30:00Z")), date(2026, 11, 6));
        let rule = Schedule {
            days: vec![Fri],
            ..weekdays(0, 15)
        };
        assert_eq!(
            next(&rule, "2026-11-05T23:00:00Z").as_deref(),
            Some("2026-11-06T00:15:00+01:00")
        );
        assert_eq!(
            next(&rule, "2026-11-05T23:30:00Z").as_deref(),
            Some("2026-11-13T00:15:00+01:00")
        );
        // A rule at 23:59 on its day, and midnight itself belongs to the day it starts.
        let rule = Schedule {
            days: vec![Thu],
            ..weekdays(23, 59)
        };
        assert_eq!(rule.on(date(2026, 11, 5)), due("2026-11-05T23:59:00+01:00"));
        let rule = Schedule {
            days: vec![Fri],
            ..weekdays(0, 0)
        };
        assert_eq!(rule.on(date(2026, 11, 6)), due("2026-11-06T00:00:00+01:00"));
    }

    #[test]
    fn daylight_saving_starts() {
        // Sunday 29 March 2026: at 02:00 clocks jump to 03:00.
        let sunday = |hour, minute| Schedule {
            days: vec![Sun],
            skip_holidays: false,
            ..weekdays(hour, minute)
        };
        assert_eq!(
            sunday(1, 59).on(date(2026, 3, 29)),
            due("2026-03-29T01:59:00+01:00")
        );
        assert_eq!(
            sunday(2, 0).on(date(2026, 3, 29)),
            due("2026-03-29T03:00:00+02:00")
        );
        assert_eq!(
            sunday(2, 30).on(date(2026, 3, 29)),
            due("2026-03-29T03:00:00+02:00")
        );
        assert_eq!(
            sunday(3, 0).on(date(2026, 3, 29)),
            due("2026-03-29T03:00:00+02:00")
        );
        assert_eq!(
            sunday(7, 40).on(date(2026, 3, 29)),
            due("2026-03-29T07:40:00+02:00")
        );
        // Seen from Saturday, in winter time, the next one is already in summer time.
        assert_eq!(
            next(&sunday(7, 40), "2026-03-28T12:00:00Z").as_deref(),
            Some("2026-03-29T07:40:00+02:00")
        );
    }

    #[test]
    fn daylight_saving_ends() {
        // Sunday 25 October 2026: at 03:00 clocks go back to 02:00, so 02:30 happens twice.
        let sunday = |hour, minute| Schedule {
            days: vec![Sun],
            skip_holidays: false,
            ..weekdays(hour, minute)
        };
        assert_eq!(
            sunday(2, 30).on(date(2026, 10, 25)),
            due("2026-10-25T02:30:00+02:00")
        );
        assert_eq!(
            sunday(3, 0).on(date(2026, 10, 25)),
            due("2026-10-25T03:00:00+01:00")
        );
        // At 02:15 of the repeated hour the first 02:30 has passed: next Sunday.
        assert_eq!(
            next(&sunday(2, 30), "2026-10-25T01:15:00Z").as_deref(),
            Some("2026-11-01T02:30:00+01:00")
        );
    }

    #[test]
    fn days_follow_the_local_calendar_across_daylight_saving() {
        // Every weekday morning of the week around 29 March: 07:40 local every day.
        let rule = weekdays(7, 40);
        assert_eq!(rule.on(date(2026, 3, 27)), due("2026-03-27T07:40:00+01:00"));
        assert_eq!(rule.on(date(2026, 3, 30)), due("2026-03-30T07:40:00+02:00"));
    }
}
