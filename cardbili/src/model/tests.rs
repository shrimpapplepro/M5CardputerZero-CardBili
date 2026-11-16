use super::*;
use serde_json::json;

#[test]
fn duration_formats_minutes_hours_and_fractional_seconds() {
    for (seconds, expected) in [
        (0.0, "0:00"),
        (9.9, "0:09"),
        (59.9, "0:59"),
        (60.0, "1:00"),
        (3599.0, "59:59"),
        (3600.0, "1:00:00"),
        (3661.0, "1:01:01"),
        (90061.0, "25:01:01"),
    ] {
        assert_eq!(mmss(seconds), expected, "seconds={seconds}");
    }
}

#[test]
fn invalid_durations_display_zero() {
    for seconds in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(mmss(seconds), "0:00");
    }
}

#[test]
fn duration_accepts_numeric_json_and_numeric_strings() {
    for (value, expected) in [
        (json!(90), 90.0),
        (json!(1.5), 1.5),
        (json!("12.5"), 12.5),
        (json!(null), 0.0),
        (json!("bad"), 0.0),
        (json!({}), 0.0),
    ] {
        assert_eq!(dur_of(&value), expected);
    }
}

#[test]
fn counts_switch_units_at_ten_thousand_and_one_hundred_million() {
    for (count, expected) in [
        (0, "0"),
        (9999, "9999"),
        (10000, "1.0万"),
        (12345, "1.2万"),
        (100_000_000, "1.0亿"),
        (250_000_000, "2.5亿"),
    ] {
        assert_eq!(fmt_num(count), expected);
    }
}

#[test]
fn calendar_handles_epoch_leap_days_and_century_rules() {
    for (days, expected) in [
        (-1, (1969, 12, 31)),
        (0, (1970, 1, 1)),
        (11016, (2000, 2, 29)),
        (19782, (2024, 2, 29)),
        (47541, (2100, 3, 1)),
    ] {
        assert_eq!(civil_from_days(days), expected, "days={days}");
    }
}

#[test]
fn dates_use_utc_plus_eight_at_midnight() {
    assert_eq!(date_of(1709135999), "2024-02-28");
    assert_eq!(date_of(1709136000), "2024-02-29");
    assert_eq!(date_of(0), "");
    assert_eq!(date_of(-1), "");
}

#[test]
fn history_labels_follow_local_calendar_days() {
    let midnight = 1709136000;
    for (timestamp, time, label) in [
        (midnight, "今天 00:00", "今天"),
        (midnight - 1, "昨天 23:59", "昨天"),
        (midnight - 86401, "02-27 23:59", "2024年2月27日"),
        (0, "", ""),
        (-1, "", ""),
    ] {
        assert_eq!(watch_time_at(timestamp, midnight), time);
        assert_eq!(history_day_label_at(timestamp, midnight), label);
    }
}
