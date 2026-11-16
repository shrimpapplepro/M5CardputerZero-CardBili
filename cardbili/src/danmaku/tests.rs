use super::*;

fn dm(t: f64, mode: u8, text: &str) -> Dm {
    Dm {
        t,
        mode,
        color: 0x123456,
        text: text.into(),
    }
}

#[test]
fn engine_filters_invalid_times_empty_text_and_unsupported_modes() {
    let engine = Engine::new(vec![
        dm(f64::NAN, 1, "nan"),
        dm(f64::INFINITY, 1, "infinite"),
        dm(-1.0, 1, "negative"),
        dm(0.0, 1, " \n"),
        dm(0.0, 7, "advanced"),
        dm(0.0, 1, "valid"),
    ]);
    assert_eq!(engine.groups.len(), 1);
    assert_eq!(engine.rows_at(0.0)[0].text, "valid");
    for time in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(engine.rows_at(time).is_empty());
    }
}

#[test]
fn engine_sorts_input_and_merges_consecutive_duplicates() {
    let engine = Engine::new(vec![
        dm(2.0, 1, "same"),
        dm(0.0, 1, "same"),
        dm(1.0, 1, "same"),
    ]);
    assert_eq!(engine.groups.len(), 1);
    let row = &engine.rows_at(0.0)[0];
    assert_eq!(row.text, "same ×3");
    assert_eq!(row.color, 0x123456);
    assert_eq!(row.x, 320.0);
}

#[test]
fn different_kinds_and_distant_duplicates_stay_separate() {
    let engine = Engine::new(vec![
        dm(0.0, 1, "same"),
        dm(0.1, 5, "same"),
        dm(2.0, 1, "same"),
    ]);
    assert_eq!(engine.groups.len(), 3);
    assert!(engine.groups.iter().all(|group| group.count == 1));
}

#[test]
fn scroll_moves_left_and_fixed_rows_stay_centered() {
    let engine = Engine::new(vec![
        dm(10.0, 1, "scroll"),
        dm(10.0, 5, "top"),
        dm(10.0, 4, "bottom"),
    ]);
    let start = engine.rows_at(10.0);
    let later = engine.rows_at(12.0);
    assert_eq!(start.len(), 3);
    assert!(later[0].x < start[0].x);
    assert_eq!(later[0].y, start[0].y);
    assert_eq!(later[1].x, start[1].x);
    assert_eq!(later[2].x, start[2].x);
    assert_eq!(start[2].y, 154.0);
    assert_eq!(engine.rows_at(14.01).len(), 1);
    assert!(engine.rows_at(16.01).is_empty());
}

#[test]
fn segment_boundary_and_backward_seek_keep_the_same_rows() {
    let engine = Engine::new(vec![dm(179.5, 1, "跨段"), dm(181.0, 5, "固定")]);
    assert_eq!(engine.rows_at(179.5)[0].text, "跨段");
    assert_eq!(engine.rows_at(180.0)[0].text, "跨段");
    assert_eq!(engine.rows_at(181.0).len(), 2);
    assert!(engine.rows_at(200.0).is_empty());
    let rewind = engine.rows_at(180.0);
    assert_eq!(rewind.len(), 1);
    assert_eq!(rewind[0].id, engine.rows_at(179.5)[0].id);
    assert!(engine.rows_at(179.49).is_empty());
}

#[test]
fn fixed_lanes_have_separate_capacity_and_become_reusable() {
    let mut items: Vec<_> = (0..10).map(|i| dm(0.0, 5, &format!("top{i}"))).collect();
    items.extend((0..6).map(|i| dm(0.0, 4, &format!("bottom{i}"))));
    items.push(dm(4.01, 5, "reused"));
    let engine = Engine::new(items);
    assert_eq!(engine.rows_at(0.0).len(), 11);
    let later = engine.rows_at(4.01);
    assert_eq!(later.len(), 1);
    assert_eq!(later[0].text, "reused");
    assert_eq!(later[0].y, 2.0);
}

#[test]
fn live_deduplicates_within_merge_window() {
    let mut engine = LiveEngine::default();
    engine.push(dm(0.0, 1, "same"), 0.0);
    engine.push(dm(0.0, 1, "same"), 1.0);
    assert_eq!(engine.rows_at(1.0).len(), 1);
    engine.push(dm(0.0, 1, "same"), 1.3);
    assert_eq!(engine.rows_at(1.3).len(), 2);
}

#[test]
fn live_returns_blocked_message_for_retry_and_respects_reserved_rows() {
    let mut engine = LiveEngine::default();
    assert!(engine.try_push(dm(0.0, 5, "first"), 0.0, 6).is_ok());
    assert_eq!(engine.rows_at(0.0)[0].y, 86.0);
    let retry = engine.try_push(dm(0.0, 5, "next"), 0.0, 6).err().unwrap();
    assert_eq!(retry.text, "next");
    assert_eq!(retry.color, 0x123456);
    assert!(engine.try_push(retry, 4.01, 6).is_ok());
    assert_eq!(engine.rows_at(4.01).len(), 1);
    assert_eq!(engine.rows_at(4.01)[0].text, "next");
}
