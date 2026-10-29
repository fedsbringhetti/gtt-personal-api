use chrono::{DateTime, Utc};
use chrono_tz::Tz;

pub const TIME_ZONE: Tz = chrono_tz::Europe::Rome;

/// Converts an upstream time to local time.
///
/// OTP gives times as seconds from `service_day`, the Unix time of noon minus 12 hours on the
/// service date (the GTFS convention). Adding the two is correct after midnight (seconds above
/// 86,400) and across daylight saving changes; "today + seconds" is not.
pub fn service_time(service_day: i64, seconds: i64) -> Option<DateTime<Tz>> {
    DateTime::from_timestamp(service_day.checked_add(seconds)?, 0)
        .map(|time| time.with_timezone(&TIME_ZONE))
}

/// Source of the current time, replaceable in tests.
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FixedClock(pub DateTime<Utc>);

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, TimeDelta, TimeZone};

    /// `serviceDay` as OTP computes it: noon on the service date, minus 12 hours.
    fn service_day(year: i32, month: u32, day: u32) -> i64 {
        let noon = NaiveDate::from_ymd_opt(year, month, day)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        let noon = TIME_ZONE.from_local_datetime(&noon).single().unwrap();
        (noon - TimeDelta::hours(12)).timestamp()
    }

    fn local(service_day: i64, hours: i64, minutes: i64) -> String {
        service_time(service_day, hours * 3600 + minutes * 60)
            .unwrap()
            .to_rfc3339()
    }

    #[test]
    fn matches_service_day_seen_upstream() {
        assert_eq!(service_day(2026, 10, 29), 1_793_228_400);
    }

    #[test]
    fn regular_day() {
        let day = service_day(2026, 10, 29);
        assert_eq!(local(day, 8, 5), "2026-10-29T08:05:00+01:00");
    }

    #[test]
    fn after_midnight_belongs_to_next_calendar_day() {
        let day = service_day(2026, 10, 29);
        assert_eq!(local(day, 25, 10), "2026-10-30T01:10:00+01:00");
    }

    #[test]
    fn spring_forward_day() {
        let day = service_day(2026, 3, 29);
        assert_eq!(local(day, 8, 0), "2026-03-29T08:00:00+02:00");
        assert_eq!(local(day, 23, 30), "2026-03-29T23:30:00+02:00");
    }

    #[test]
    fn fall_back_day() {
        let day = service_day(2026, 10, 25);
        assert_eq!(local(day, 8, 0), "2026-10-25T08:00:00+01:00");
        assert_eq!(local(day, 23, 30), "2026-10-25T23:30:00+01:00");
    }

    #[test]
    fn overflow_returns_none() {
        assert!(service_time(i64::MAX, 1).is_none());
    }
}
