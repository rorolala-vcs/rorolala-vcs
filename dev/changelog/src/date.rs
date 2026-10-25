//! Today's UTC date, as a heading writes it.
//!
//! An archived version is dated in UTC rather than in the local zone, so the date a release carries
//! is the same date on every machine that archives it. The civil calendar is worked out from the
//! Unix day count: a date library for one date would be a dependency carried for arithmetic that
//! fits in a dozen lines.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::Error;

/// The current UTC date, as `YYYY-MM-DD`.
///
/// # Errors
///
/// Reports [`Error::Clock`] when the system clock is before the Unix epoch, which is the one case
/// the day count does not describe.
pub fn today_utc() -> Result<String, Error> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::Clock)?
        .as_secs();

    let days = i64::try_from(seconds / 86_400).map_err(|_| Error::Clock)?;

    Ok(civil_from_days(days))
}

/// The civil date `days` days after 1970-01-01, as `YYYY-MM-DD`.
///
/// Howard Hinnant's `civil_from_days`: the era is a 400-year cycle, and the month is counted from
/// March so that the leap day is the last day of the year and needs no special case.
fn civil_from_days(days: i64) -> String {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    let year = if month <= 2 { year + 1 } else { year };

    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::civil_from_days;

    #[test]
    fn the_epoch_is_the_first_of_january_1970() {
        assert_eq!(civil_from_days(0), "1970-01-01");
        assert_eq!(civil_from_days(1), "1970-01-02");
    }

    #[test]
    fn a_leap_day_is_the_twenty_ninth_of_february() {
        // 2000-02-29 is 11_016 days after the epoch.
        assert_eq!(civil_from_days(11_016), "2000-02-29");
    }

    #[test]
    fn an_ordinary_date_is_read_from_the_day_count() {
        // 2026-10-25 is 20_751 days after the epoch.
        assert_eq!(civil_from_days(20_751), "2026-10-25");
        assert_eq!(civil_from_days(20_752), "2026-10-26");
    }

    #[test]
    fn the_year_end_rolls_over() {
        // 2025-12-31 is 20_453 days after the epoch; the next day is 2026-01-01.
        assert_eq!(civil_from_days(20_453), "2025-12-31");
        assert_eq!(civil_from_days(20_454), "2026-01-01");
    }
}
