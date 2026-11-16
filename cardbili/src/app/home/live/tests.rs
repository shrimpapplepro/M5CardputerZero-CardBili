use super::*;

#[test]
fn grouped_grid_starts_each_section_on_its_own_row() {
    let state = LiveState {
        following_count: 4,
        ..Default::default()
    };
    let layout = state.layout(26, 3, 72.0, 22.0);
    assert_eq!(
        layout
            .headers
            .iter()
            .map(|h| h.title.as_str())
            .collect::<Vec<_>>(),
        ["我的关注", "热门直播", "全部直播"]
    );
    assert_eq!(
        layout.headers.iter().map(|h| h.top).collect::<Vec<_>>(),
        [0.0, 166.0, 692.0]
    );
    assert_eq!(&layout.tops[..4], &[22.0, 22.0, 22.0, 94.0]);
    assert_eq!(layout.tops[4], 188.0);
    assert_eq!(layout.tops[24], 714.0);
    assert_eq!(layout.selection_top(4), 166.0);
    assert_eq!(layout.selection_top(7), 260.0);
    assert_eq!(state.column(4, 3), 0);
}

#[test]
fn empty_sections_have_no_headers_or_height() {
    let state = LiveState::default();
    let layout = state.layout(2, 3, 72.0, 22.0);
    assert_eq!(layout.headers.len(), 1);
    assert_eq!(layout.headers[0].title, "热门直播");
    assert_eq!(layout.tops, [22.0, 22.0]);
    let empty = state.layout(0, 3, 72.0, 22.0);
    assert!(empty.headers.is_empty() && empty.tops.is_empty());
}

#[test]
fn vertical_navigation_preserves_columns_across_section_boundaries() {
    let state = LiveState {
        following_count: 4,
        ..Default::default()
    };
    for (selection, step, expected) in [
        (1, 3, 3),
        (3, 3, 4),
        (4, -3, 3),
        (6, -3, 3),
        (0, -3, 0),
        (23, 3, 25),
        (24, -3, 22),
    ] {
        assert_eq!(
            state.move_vertical(selection, step),
            expected,
            "selection={selection}, step={step}"
        );
    }
}
