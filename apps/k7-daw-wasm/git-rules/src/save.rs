// save.rs — the moment a Save is recorded under, as the canvas writes it (23/11/2026 00:14).
// A reader would get wrong that the time zone is the page's: the caller says how many minutes its clock stands from UTC.

/// Days from 1970-01-01 to a civil date's day, and back: the proleptic Gregorian calendar.
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// `dd/mm/yyyy hh:mm` of an instant on a clock `offset_min` minutes ahead of UTC.
#[must_use]
pub fn save_when(epoch_ms: i64, offset_min: i32) -> String {
    let local_min = epoch_ms.div_euclid(60_000) + i64::from(offset_min);
    let (y, m, d) = civil(local_min.div_euclid(1_440));
    let in_day = local_min.rem_euclid(1_440);
    format!("{d:02}/{m:02}/{y} {:02}:{:02}", in_day / 60, in_day % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_the_moment_as_the_canvas_does() {
        // 2026-11-23 00:14 UTC.
        assert_eq!(save_when(1_795_392_840_000, 0), "23/11/2026 00:14");
    }

    #[test]
    fn follows_the_clock_of_the_caller() {
        assert_eq!(save_when(1_795_392_840_000, 60), "23/11/2026 01:14");
        assert_eq!(save_when(1_795_392_840_000, -60), "22/11/2026 23:14");
    }

    #[test]
    fn handles_the_epoch_and_a_leap_day() {
        assert_eq!(save_when(0, 0), "01/01/1970 00:00");
        assert_eq!(save_when(951_782_400_000, 0), "29/02/2000 00:00");
        assert_eq!(save_when(-60_000, 0), "31/12/1969 23:59");
    }
}
