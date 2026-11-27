// format_at.rs — a position in the song as the transport clock writes it: minutes and seconds, the hour only when there is one.

/// Seconds as `m:ss`, or `h:mm:ss` from an hour; floored and never negative.
pub fn format_at(seconds: f64) -> String {
    let s = if seconds.is_finite() {
        seconds.floor().max(0.0)
    } else {
        0.0
    };
    let m = (s / 60.0).floor() % 60.0;
    let h = (s / 3600.0).floor();
    let sec = s % 60.0;
    if h > 0.0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m}:{sec:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_minutes_and_seconds_and_the_hour_only_when_there_is_one() {
        assert_eq!(format_at(0.0), "0:00");
        assert_eq!(format_at(83.7), "1:23");
        assert_eq!(format_at(3723.0), "1:02:03");
    }

    #[test]
    fn floors_and_clamps() {
        assert_eq!(format_at(119.9), "1:59");
        assert_eq!(format_at(-2.0), "0:00");
    }
}
