use super::*;

fn item(id: &str) -> model::Item {
    model::Item {
        bvid: id.into(),
        ..Default::default()
    }
}

#[test]
fn scroll_moves_only_as_needed_and_clamps_to_content() {
    for (scroll, top, bottom, viewport, content, expected) in [
        (30.0, 40.0, 60.0, 100.0, 300.0, 30.0),
        (80.0, 20.0, 40.0, 100.0, 300.0, 20.0),
        (0.0, 180.0, 210.0, 100.0, 300.0, 110.0),
        (200.0, 10.0, 30.0, 100.0, 50.0, 0.0),
        (900.0, 280.0, 310.0, 100.0, 300.0, 200.0),
    ] {
        assert_eq!(
            scroll_to_visible(scroll, top, bottom, viewport, content),
            expected
        );
    }
}

#[test]
fn visible_row_helpers_handle_first_last_and_small_viewports() {
    for (first, selected, visible, expected) in [
        (3, 4, 5, 3),
        (3, 2, 5, 2),
        (3, 8, 5, 4),
        (0, 0, 0, 0),
        (0, 5, 1, 5),
    ] {
        assert_eq!(ensure_visible(first, selected, visible), expected);
    }
    assert_eq!(visible_rows(126.0, 4.0, 24.0), 5);
    assert_eq!(visible_rows(10.0, 4.0, 24.0), 1);
}

#[test]
fn first_page_refresh_keeps_unique_tail_and_matching_covers() {
    let mut items = vec![item("a"), item("b"), item("c"), item("d"), item("d")];
    let mut covers = vec![
        None,
        Some(util::raw_to_image(1, 1, &[1, 2, 3])),
        None,
        Some(util::raw_to_image(2, 1, &[0; 6])),
        None,
    ];
    let mut size = 2;
    let selection = refresh_first_page(
        &mut items,
        &mut covers,
        &mut size,
        vec![item("b"), item("c"), item("new")],
        2,
    );
    assert_eq!(
        items
            .iter()
            .map(|item| item.bvid.as_str())
            .collect::<Vec<_>>(),
        ["b", "c", "new", "d"]
    );
    assert_eq!(size, 3);
    assert_eq!(selection, 2);
    assert_eq!(covers.len(), items.len());
    let size = covers[0].as_ref().unwrap().size();
    assert_eq!((size.width, size.height), (1, 1));
    assert!(covers[1].is_none() && covers[2].is_none());
}

#[test]
fn first_page_refresh_clamps_selection_after_shrink_or_empty_result() {
    let mut items = vec![item("a"), item("b")];
    let mut covers = vec![None; 2];
    let mut size = 9;
    assert_eq!(
        refresh_first_page(&mut items, &mut covers, &mut size, vec![item("a")], 10),
        0
    );
    assert_eq!(
        refresh_first_page(&mut items, &mut covers, &mut size, vec![], -1),
        0
    );
    assert!(items.is_empty() && covers.is_empty());
    assert_eq!(size, 0);
}

#[test]
fn compact_counts_drop_fraction_only_for_large_unit_values() {
    for (value, expected) in [
        ("123.4万", "123万"),
        ("100.0亿", "100亿"),
        ("12.3万", "12.3万"),
        ("123.4", "123.4"),
        ("--", "--"),
        ("", ""),
    ] {
        assert_eq!(compact_card_count(value).as_str(), expected);
    }
}

#[test]
fn tab_storage_indices_and_display_order_remain_distinct() {
    assert_eq!(HomeTab::Live.index(), 5);
    assert_eq!(HomeTab::Dynamic.cycle(1), HomeTab::Live);
    assert_eq!(HomeTab::Live.cycle(1), HomeTab::Mine);
    assert_eq!(HomeTab::Recommended.cycle(-1), HomeTab::Settings);
    assert_eq!(HomeTab::Settings.cycle(1), HomeTab::Recommended);
    for index in 0..6 {
        assert_eq!(HomeTab::from_index(index).index(), index);
    }
    assert_eq!(list_source(HomeTab::Mine, 3), Some(3));
    assert_eq!(list_source(HomeTab::Mine, -1), None);
    assert_eq!(list_source(HomeTab::Settings, -1), None);
}
