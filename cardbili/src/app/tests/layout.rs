use super::*;

#[test]
fn slint_geometry_reserves_navigation_and_bottom_bar_space() {
    let mut h = Harness::new();
    for (position, expected) in [
        (MenuPosition::Top, (0.0, 24.0, 320.0, 126.0)),
        (MenuPosition::Left, (56.0, 0.0, 264.0, 150.0)),
        (MenuPosition::Right, (0.0, 0.0, 264.0, 150.0)),
    ] {
        h.app.settings.menu_position = position;
        h.sync();
        let layout = h.app.ui.global::<crate::Layout>();
        assert_eq!(
            (
                layout.get_video_x(),
                layout.get_video_y(),
                layout.get_video_width(),
                layout.get_video_height()
            ),
            expected
        );
        assert_eq!(layout.get_bottom_bar_height(), 20.0);
    }
    h.app.page = Page::Collection;
    h.sync();
    let layout = h.app.ui.global::<crate::Layout>();
    assert_eq!(
        (
            layout.get_sidebar_width(),
            layout.get_video_y(),
            layout.get_video_height()
        ),
        (0.0, 30.0, 120.0)
    );
}

#[test]
fn grid_geometry_matches_column_count_and_selected_card_visibility() {
    let mut h = Harness::new();
    h.videos(20);
    for (layout, columns, cell_height) in [
        (VideoLayout::ThreeColumns, 3, 84.0),
        (VideoLayout::TwoColumns, 2, 113.0),
    ] {
        h.app.view = layout;
        h.app.sel = 13;
        h.sync();
        let list = h.app.ui.global::<crate::ListModel>();
        assert_eq!(list.get_cols(), columns);
        assert!((list.get_cell_w() * columns as f32 - 320.0).abs() < 0.001);
        assert_eq!(list.get_cell_h(), cell_height);
        let active = list.get_grid_active() as usize;
        assert_eq!(
            list.get_grid_rows().row_data(active).unwrap().title,
            "视频 BV13"
        );
        let top = list.get_grid_tops().row_data(active).unwrap();
        let offset = list.get_grid_offset();
        assert!(top >= offset);
        assert!(
            top + cell_height <= offset + h.app.ui.global::<crate::Layout>().get_video_height()
        );
    }
}

#[test]
fn list_scroll_keeps_selected_row_visible_and_resets_with_layout() {
    let mut h = Harness::new();
    h.videos(20);
    h.app.view = VideoLayout::List;
    h.app.sel = 10;
    h.sync();
    assert_eq!(h.app.list_scroll, 204.0);
    assert_eq!(
        h.app.ui.global::<crate::ListModel>().get_rows().row_count(),
        20
    );
    h.app.sel = 0;
    h.sync();
    assert_eq!(h.app.list_scroll, 0.0);
    h.app.view = VideoLayout::TwoColumns;
    h.sync();
    assert_eq!(h.app.grid_scroll, 0.0);
}

#[test]
fn empty_list_has_no_active_card_or_scroll() {
    let mut h = Harness::new();
    h.app.sel = 99;
    h.sync();
    assert_eq!(h.app.sel, 0);
    let list = h.app.ui.global::<crate::ListModel>();
    assert_eq!(list.get_grid_active(), -1);
    assert_eq!(list.get_grid_rows().row_count(), 0);
    assert_eq!(list.get_grid_offset(), 0.0);
}

#[test]
fn cards_without_statistics_and_dated_list_rows_use_correct_heights() {
    let mut h = Harness::new();
    h.videos(1);
    h.app.lists[0][0].views = None;
    h.sync();
    assert_eq!(h.app.ui.global::<crate::ListModel>().get_cell_h(), 72.0);
    h.app.view = VideoLayout::List;
    h.app.lists[0][0].date = Some("2024-02-29".into());
    h.app.home_dirty = true;
    h.sync();
    assert_eq!(h.app.ui.global::<crate::ListModel>().get_row_height(), 42.0);
}

#[test]
fn settings_scroll_keeps_selected_group_header_visible() {
    let mut h = Harness::new();
    h.app.tab = HomeTab::Settings;
    let appearance = h
        .app
        .ui
        .global::<crate::SettingsPage>()
        .get_appearance_items()
        .row_count();
    h.app.sel = appearance as i32;
    h.sync();
    let first = h.app.ui.global::<crate::SettingsPage>().get_first_line();
    assert!(first <= appearance as i32 + 1);
    assert!(first + 5 > appearance as i32 + 2);
}

#[test]
fn software_renderer_draws_pages_and_menu_slots_without_a_display() {
    let mut h = Harness::new();
    for page in [
        Page::Home,
        Page::Detail,
        Page::About,
        Page::Login,
        Page::Comments,
        Page::Following,
        Page::Favorites,
        Page::Collection,
        Page::Player,
    ] {
        h.app.page = page;
        h.sync();
        let pixels = h.render();
        assert_eq!(pixels.len(), 320 * 170);
        assert!(pixels.iter().any(|pixel| pixel.r > 100), "page={page:?}");
    }
    h.app.page = Page::Home;
    h.app.tab = HomeTab::Settings;
    h.sync();
    let pixels = h.render();
    for (slot, center) in [49usize, 106, 162, 217, 275].into_iter().enumerate() {
        let text_pixels = (150..170)
            .flat_map(|y| (center - 27..center + 27).map(move |x| y * 320 + x))
            .filter(|&index| pixels[index].r > 100)
            .count();
        assert_eq!(text_pixels > 0, slot == 4, "slot={slot}");
    }
}
