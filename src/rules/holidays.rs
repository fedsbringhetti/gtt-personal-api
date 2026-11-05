//! Public holidays that rules can skip (Phase 3, decision K): Italian national holidays and
//! 24 June, San Giovanni, the patron saint of Turin.

use chrono::{Datelike, NaiveDate, TimeDelta};

/// First year in which 4 October (San Francesco) is a national holiday again (law 151/2025).
const SAN_FRANCESCO_FROM: i32 = 2026;

/// The holiday on `date`, by its Italian name; `None` on any other day.
pub fn holiday(date: NaiveDate) -> Option<&'static str> {
    let fixed = match (date.month(), date.day()) {
        (1, 1) => Some("Capodanno"),
        (1, 6) => Some("Epifania"),
        (4, 25) => Some("Festa della Liberazione"),
        (5, 1) => Some("Festa del Lavoro"),
        (6, 2) => Some("Festa della Repubblica"),
        (6, 24) => Some("San Giovanni"),
        (8, 15) => Some("Ferragosto"),
        (10, 4) if date.year() >= SAN_FRANCESCO_FROM => Some("San Francesco"),
        (11, 1) => Some("Ognissanti"),
        (12, 8) => Some("Immacolata"),
        (12, 25) => Some("Natale"),
        (12, 26) => Some("Santo Stefano"),
        _ => None,
    };
    fixed.or_else(|| {
        let easter = easter(date.year())?;
        if date == easter {
            Some("Pasqua")
        } else if date == easter + TimeDelta::days(1) {
            Some("Pasquetta")
        } else {
            None
        }
    })
}

/// Easter Sunday in the Gregorian calendar (the anonymous algorithm, also known as Meeus/Jones/
/// Butcher): a fixed sequence of integer operations, valid for every Gregorian year.
pub fn easter(year: i32) -> Option<NaiveDate> {
    let a = year.rem_euclid(19);
    let b = year.div_euclid(100);
    let c = year.rem_euclid(100);
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    NaiveDate::from_ymd_opt(year, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    #[test]
    fn easter_dates() {
        let known = [
            (2024, 3, 31),
            (2025, 4, 20),
            (2026, 4, 5),
            (2027, 3, 28),
            (2028, 4, 16),
            (2029, 4, 1),
            (2030, 4, 21),
            // The earliest and latest possible dates.
            (2285, 3, 22),
            (1943, 4, 25),
        ];
        for (year, month, day) in known {
            assert_eq!(easter(year), Some(date(year, month, day)), "{year}");
        }
    }

    #[test]
    fn holidays_of_2026() {
        let holidays: Vec<(NaiveDate, &str)> = date(2026, 1, 1)
            .iter_days()
            .take_while(|day| day.year() == 2026)
            .filter_map(|day| holiday(day).map(|name| (day, name)))
            .collect();
        assert_eq!(
            holidays,
            [
                (date(2026, 1, 1), "Capodanno"),
                (date(2026, 1, 6), "Epifania"),
                (date(2026, 4, 5), "Pasqua"),
                (date(2026, 4, 6), "Pasquetta"),
                (date(2026, 4, 25), "Festa della Liberazione"),
                (date(2026, 5, 1), "Festa del Lavoro"),
                (date(2026, 6, 2), "Festa della Repubblica"),
                (date(2026, 6, 24), "San Giovanni"),
                (date(2026, 8, 15), "Ferragosto"),
                (date(2026, 10, 4), "San Francesco"),
                (date(2026, 11, 1), "Ognissanti"),
                (date(2026, 12, 8), "Immacolata"),
                (date(2026, 12, 25), "Natale"),
                (date(2026, 12, 26), "Santo Stefano"),
            ]
        );
    }

    #[test]
    fn san_francesco_only_from_2026() {
        assert_eq!(holiday(date(2025, 10, 4)), None);
        assert_eq!(holiday(date(2027, 10, 4)), Some("San Francesco"));
    }

    #[test]
    fn easter_monday_moves_with_easter() {
        assert_eq!(holiday(date(2027, 3, 29)), Some("Pasquetta"));
        assert_eq!(holiday(date(2026, 3, 29)), None);
        // In 2011 Easter Monday fell on 25 April: one holiday, under its fixed name.
        assert_eq!(holiday(date(2011, 4, 25)), Some("Festa della Liberazione"));
    }

    #[test]
    fn ordinary_days() {
        assert_eq!(holiday(date(2026, 11, 2)), None);
        assert_eq!(holiday(date(2026, 12, 24)), None);
        assert_eq!(holiday(date(2026, 8, 14)), None);
    }
}
