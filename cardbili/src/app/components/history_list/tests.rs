use super::*;

#[test]
fn progress_distinguishes_unwatched_complete_and_unknown_duration() {
    for (progress, duration_secs, expected) in [
        (0, 100.0, ""),
        (-1, 100.0, "已看完"),
        (99, 100.0, "已看完"),
        (110, 100.0, "已看完"),
        (61, 100.0, "01:01 / 01:40"),
        (3661, 0.0, "1:01:01"),
    ] {
        let item = model::Item {
            progress,
            duration_secs,
            ..Default::default()
        };
        assert_eq!(progress_text(&item).as_str(), expected);
    }
}

#[test]
fn history_headers_group_by_utc_plus_eight_day() {
    let items: Vec<_> = [1709136001, 1709136000, 1709135999, 0]
        .into_iter()
        .map(|view_at| model::Item {
            view_at,
            ..Default::default()
        })
        .collect();
    let rows = build_history_rows(&items, 30.0);
    assert_eq!(rows.rows.len(), 4);
    assert_eq!(rows.tops, [30.0, 60.0, 120.0, 150.0]);
    assert_eq!(rows.headers.len(), 2);
    assert_eq!((rows.headers[0].top, rows.headers[1].top), (0.0, 90.0));
    let empty = build_history_rows(&[], 30.0);
    assert!(empty.rows.is_empty() && empty.headers.is_empty() && empty.tops.is_empty());
}
