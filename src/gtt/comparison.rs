//! How GTT's predictions compare with OpenTripPlanner's, today (Phase 3, decision AC).

use std::{collections::HashMap, sync::Mutex};

use chrono::{NaiveDate, TimeDelta};
use serde::Serialize;

/// What happened to GTT's prediction for one run at one stop.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Outcome {
    /// Used. `difference` is GTT's expected time minus OpenTripPlanner's, when OpenTripPlanner
    /// had real-time data too.
    Used { difference: Option<TimeDelta> },
    /// Implausible, so OpenTripPlanner was used instead.
    Mismatch,
}

/// Today's outcomes, one per run and stop: reloading a page replaces the outcome, it does not
/// add another. Cleared when the day (Europe/Rome) changes.
#[derive(Debug, Default)]
pub struct Comparisons {
    day: Mutex<Day>,
}

#[derive(Debug, Default)]
struct Day {
    date: Option<NaiveDate>,
    /// Keyed by trip id and stop id.
    outcomes: HashMap<(String, String), Outcome>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ComparisonSummary {
    pub date: NaiveDate,
    /// Departures that used GTT's prediction.
    pub used: usize,
    /// Departures whose GTT prediction was implausible.
    pub mismatches: usize,
    /// Departures that used GTT's prediction while OpenTripPlanner had one too.
    pub compared: usize,
    /// Median of the absolute differences of `compared`, in seconds.
    pub median_difference_seconds: Option<i64>,
}

impl Comparisons {
    pub fn record(&self, today: NaiveDate, trip_id: &str, stop_id: &str, outcome: Outcome) {
        let mut day = self.lock(today);
        day.outcomes
            .insert((trip_id.to_owned(), stop_id.to_owned()), outcome);
    }

    pub fn summary(&self, today: NaiveDate) -> ComparisonSummary {
        let day = self.lock(today);
        let mut differences = Vec::new();
        let (mut used, mut mismatches) = (0, 0);
        for outcome in day.outcomes.values() {
            match outcome {
                Outcome::Used { difference } => {
                    used += 1;
                    differences.extend(difference.map(|d| d.num_seconds().abs()));
                }
                Outcome::Mismatch => mismatches += 1,
            }
        }
        differences.sort_unstable();
        ComparisonSummary {
            date: today,
            used,
            mismatches,
            compared: differences.len(),
            median_difference_seconds: differences.get(differences.len() / 2).copied(),
        }
    }

    /// The day's data, cleared first if `today` is a new day.
    fn lock(&self, today: NaiveDate) -> std::sync::MutexGuard<'_, Day> {
        let mut day = self
            .day
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if day.date != Some(today) {
            day.date = Some(today);
            day.outcomes.clear();
        }
        day
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, d).unwrap()
    }

    fn used(seconds: i64) -> Outcome {
        Outcome::Used {
            difference: Some(TimeDelta::seconds(seconds)),
        }
    }

    #[test]
    fn counts_each_run_and_stop_once() {
        let comparisons = Comparisons::default();
        comparisons.record(day(4), "1U", "gtt:39", used(100));
        comparisons.record(day(4), "1U", "gtt:39", used(-30)); // a reload: replaces
        comparisons.record(day(4), "1U", "gtt:40", used(60));
        comparisons.record(day(4), "2U", "gtt:39", used(-200));
        comparisons.record(day(4), "3U", "gtt:39", Outcome::Used { difference: None });
        comparisons.record(day(4), "4U", "gtt:39", Outcome::Mismatch);

        assert_eq!(
            comparisons.summary(day(4)),
            ComparisonSummary {
                date: day(4),
                used: 4,
                mismatches: 1,
                compared: 3,
                median_difference_seconds: Some(60),
            }
        );
    }

    #[test]
    fn starts_again_every_day() {
        let comparisons = Comparisons::default();
        comparisons.record(day(4), "1U", "gtt:39", Outcome::Mismatch);
        let summary = comparisons.summary(day(5));
        assert_eq!((summary.used, summary.mismatches), (0, 0));
        assert_eq!(summary.median_difference_seconds, None);
    }
}
