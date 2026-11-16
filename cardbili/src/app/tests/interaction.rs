use super::*;

#[test]
fn home_physical_keys_request_login_play_and_detail() {
    let mut h = Harness::new();
    h.videos(3);
    h.app.sel = 1;
    h.key("5");
    assert!(h.app.requested_login);
    h.key("6");
    assert_eq!(h.app.requested_play.as_deref(), Some("BV1"));
    h.key("7");
    assert_eq!(
        h.app.requested_detail,
        Some(("BV1".into(), "视频 BV1".into()))
    );
}

#[test]
fn empty_slots_and_empty_lists_leave_requests_unchanged() {
    let mut h = Harness::new();
    h.app.cookies = Some("DedeUserID=1".into());
    for key in ["5", "6", "7"] {
        h.key(key);
    }
    assert!(!h.app.requested_login);
    assert!(h.app.requested_play.is_none());
    assert!(h.app.requested_detail.is_none());
    h.app.tab = HomeTab::Settings;
    h.key("4");
    assert!(!h.app.settings_changed);
}

#[test]
fn grid_navigation_uses_columns_and_stays_within_items() {
    let mut h = Harness::new();
    h.videos(8);
    for (layout, columns) in [
        (VideoLayout::List, 1),
        (VideoLayout::TwoColumns, 2),
        (VideoLayout::ThreeColumns, 3),
    ] {
        h.app.view = layout;
        h.app.sel = 0;
        h.key("DOWN");
        assert_eq!(h.app.sel, columns);
        h.key("RIGHT");
        assert_eq!(h.app.sel, columns + 1);
        h.key("UP");
        assert_eq!(h.app.sel, 1);
        h.key("LEFT");
        h.key("LEFT");
        assert_eq!(h.app.sel, 0);
        h.app.sel = 7;
        h.key("DOWN");
        h.key("RIGHT");
        assert_eq!(h.app.sel, 7);
    }
}

#[test]
fn keyboard_aliases_and_release_events_share_navigation_state() {
    let mut h = Harness::new();
    h.videos(9);
    h.key("x");
    assert_eq!(h.app.sel, 3);
    assert!(h.app.direction_hold.is_some());
    h.key("DOWN_UP");
    assert!(h.app.direction_hold.is_none());
    h.key("c");
    assert_eq!(h.app.sel, 4);
    h.key("f");
    assert_eq!(h.app.sel, 1);
    h.key("z");
    assert_eq!(h.app.sel, 0);
}

#[test]
fn layout_and_navigation_position_shortcuts_cycle_and_mark_settings_dirty() {
    let mut h = Harness::new();
    for expected in [
        VideoLayout::TwoColumns,
        VideoLayout::List,
        VideoLayout::ThreeColumns,
    ] {
        h.app.settings_changed = false;
        h.key("v");
        assert_eq!(h.app.view, expected);
        assert!(h.app.settings_changed);
    }
    for expected in [MenuPosition::Left, MenuPosition::Right, MenuPosition::Top] {
        h.key("b");
        assert_eq!(h.app.settings.menu_position, expected);
    }
}

#[test]
fn return_key_dispatches_on_release_and_long_hold_requests_home() {
    let mut h = Harness::new();
    h.app.page = Page::Detail;
    h.key("8");
    assert!(!h.app.go_back);
    h.key("7_UP");
    assert!(h.app.return_hold.is_some());
    h.key("8_UP");
    assert!(h.app.go_back);
    h.app.go_back = false;
    h.key("8");
    h.app.return_hold = Some((8, Instant::now() - Duration::from_secs(1)));
    h.app.tick_input();
    assert!(h.app.return_home);
    h.key("8_UP");
    assert!(!h.app.go_back);
}

#[test]
fn help_blocks_background_actions_until_dismissed() {
    let mut h = Harness::new();
    h.videos(3);
    h.key("h");
    for key in ["ENTER", "TAB", "v", "b", "DOWN"] {
        h.key(key);
    }
    assert!(h.app.requested_play.is_none());
    assert_eq!(h.app.tab, HomeTab::Recommended);
    assert_eq!(h.app.view, VideoLayout::ThreeColumns);
    assert_eq!(h.app.settings.menu_position, MenuPosition::Top);
    assert_eq!(h.app.sel, 0);
    h.key("ESC");
    assert!(!h.app.help);
}

#[test]
fn hidden_player_controls_consume_first_menu_press() {
    let mut h = Harness::new();
    h.app.page = Page::Player;
    h.app
        .ui
        .global::<crate::Player>()
        .set_controls_visible(false);
    let enabled = h.app.controls.danmaku_enabled;
    h.key("7");
    assert_eq!(h.app.controls.danmaku_enabled, enabled);
    h.sync();
    assert!(h.app.ui.global::<crate::Player>().get_controls_visible());
    h.key("7");
    assert_eq!(h.app.controls.danmaku_enabled, !enabled);
}

#[test]
fn player_callback_uses_the_same_comment_action_as_physical_key() {
    let mut h = Harness::new();
    h.app.page = Page::Player;
    h.app.current_play = Some(model::PlayInfo {
        aid: 42,
        ..Default::default()
    });
    h.app.ui.global::<crate::Player>().invoke_key("r".into());
    h.dispatch_callbacks();
    assert_eq!(h.app.requested_comments, Some(42));
    h.app.requested_comments = None;
    h.key("8");
    assert_eq!(h.app.requested_comments, Some(42));
}

#[test]
fn live_refresh_and_play_request_current_room() {
    let mut h = Harness::new();
    h.app.page = Page::Player;
    h.app.live = Some(model::LiveInfo {
        room_id: 42,
        ..Default::default()
    });
    h.app.ui.global::<crate::Player>().set_live(true);
    h.key("5");
    assert_eq!(h.app.requested_live, Some(42));
    h.app.requested_live = None;
    h.key("4");
    assert_eq!(h.app.requested_live, Some(42));
}

#[test]
fn detail_buttons_request_play_author_and_related_videos() {
    let mut h = Harness::new();
    h.app.page = Page::Detail;
    h.app.detail_bvid = "BV-detail".into();
    h.app.detail_display = Some(DetailDisplay {
        data: model::Detail {
            owner: model::Owner {
                mid: "123".into(),
                name: "作者".into(),
            },
            ..Default::default()
        },
        cover: None,
    });
    h.key("5");
    assert_eq!(h.app.requested_play.as_deref(), Some("BV-detail"));
    h.key("6");
    assert_eq!(h.app.requested_uploader.as_ref().unwrap().mid, "123");
    h.key("7");
    assert_eq!(h.app.requested_related.as_deref(), Some("BV-detail"));
}

#[test]
fn cdn_panel_consumes_navigation_and_saves_selected_option() {
    let mut h = Harness::new();
    h.app.ui.global::<crate::Cdn>().set_open(true);
    h.app.ui.global::<crate::Cdn>().set_selected(0);
    h.key("UP");
    assert_eq!(h.app.ui.global::<crate::Cdn>().get_selected(), 0);
    h.key("RIGHT");
    assert_eq!(h.app.ui.global::<crate::Cdn>().get_selected(), 3);
    assert_eq!(h.app.sel, 0);
    h.key("5");
    assert_eq!(h.app.settings.cdn, "alib");
    assert!(h.app.settings_changed);
    assert!(!h.app.ui.global::<crate::Cdn>().get_open());
    assert!(!h.app.requested_login);
}

#[test]
fn sections_choose_part_or_seek_to_chapter() {
    let mut h = Harness::new();
    h.app.page = Page::Detail;
    h.app.sections = sections::Sections {
        bvid: "BV-parts".into(),
        cid: 10,
        open: true,
        parts: vec![
            model::VideoPart {
                cid: 10,
                title: "P1".into(),
                duration: 60.0,
            },
            model::VideoPart {
                cid: 20,
                title: "P2".into(),
                duration: 60.0,
            },
        ],
        chapters: vec![model::Chapter {
            title: "章节".into(),
            start: 12.0,
        }],
        ..Default::default()
    };
    h.key("DOWN");
    h.key("5");
    assert_eq!(h.app.requested_cid, Some(20));
    assert_eq!(h.app.requested_play.as_deref(), Some("BV-parts"));
    assert_eq!(h.app.play_start, 0.0);
    assert!(!h.app.sections.open);
    h.app.page = Page::Player;
    h.app.current_play = Some(model::PlayInfo {
        cid: 10,
        ..Default::default()
    });
    h.app.requested_play = None;
    h.app.sections.open = true;
    h.app.sections.selection = 2;
    h.key("5");
    assert_eq!(h.app.seek_pending, Some(12.0));
    assert!(h.app.seek_deadline.is_some());
    assert!(h.app.requested_play.is_none());
}

fn select_setting(h: &mut Harness, item: crate::SettingsItem) {
    h.app.tab = HomeTab::Settings;
    let page = h.app.ui.global::<crate::SettingsPage>();
    h.app.sel = page
        .get_appearance_items()
        .iter()
        .chain(page.get_general_items().iter())
        .chain(page.get_advanced_items().iter())
        .position(|row| row.item == item)
        .unwrap() as i32;
}

#[test]
fn settings_switches_and_opacity_limits_update_state() {
    let mut h = Harness::new();
    select_setting(&mut h, crate::SettingsItem::Danmaku);
    h.key("ENTER");
    assert!(!h.app.settings.danmaku_enabled);
    select_setting(&mut h, crate::SettingsItem::DanmakuOpacity);
    h.app.settings.danmaku_opacity = 100;
    h.app.settings_changed = false;
    h.key("RIGHT");
    assert_eq!(h.app.settings.danmaku_opacity, 100);
    assert!(!h.app.settings_changed);
    h.key("LEFT");
    assert_eq!(h.app.settings.danmaku_opacity, 90);
    h.app.settings.danmaku_opacity = 0;
    h.key("LEFT");
    assert_eq!(h.app.settings.danmaku_opacity, 0);
}

#[test]
fn pixel_optimization_disables_algorithm_changes() {
    let mut h = Harness::new();
    select_setting(&mut h, crate::SettingsItem::PixelOptimize);
    h.key("SPACE");
    assert!(h.app.settings.pixel_optimize);
    select_setting(&mut h, crate::SettingsItem::ScaleAlgorithm);
    let algorithm = h.app.settings.scale_algorithm;
    h.key("RIGHT");
    assert_eq!(h.app.settings.scale_algorithm, algorithm);
    h.app.settings.pixel_optimize = false;
    h.key("RIGHT");
    assert_eq!(h.app.settings.scale_algorithm, algorithm.cycle(1));
}

#[test]
fn settings_navigation_wraps_and_about_requests_navigation() {
    let mut h = Harness::new();
    h.app.tab = HomeTab::Settings;
    h.key("UP");
    assert!(h.app.sel > 0);
    h.key("DOWN");
    assert_eq!(h.app.sel, 0);
    select_setting(&mut h, crate::SettingsItem::About);
    h.key("ENTER");
    assert!(h.app.requested_about);
    h.key("ESC");
    assert_eq!(h.app.tab, HomeTab::Recommended);
    assert_eq!(h.app.sel, 0);
}

#[test]
fn confirmation_defaults_to_cancel_and_intercepts_background_input() {
    let mut h = Harness::new();
    select_setting(&mut h, crate::SettingsItem::Cache);
    h.key("ENTER");
    assert!(h.app.ui.global::<crate::Confirm>().get_open());
    assert!(!h.app.ui.global::<crate::Confirm>().get_selected());
    let selection = h.app.sel;
    h.key("RIGHT");
    assert!(h.app.ui.global::<crate::Confirm>().get_selected());
    h.key("TAB");
    assert!(!h.app.ui.global::<crate::Confirm>().get_selected());
    h.key("DOWN");
    assert_eq!(h.app.sel, selection);
    h.key("ENTER");
    assert!(!h.app.ui.global::<crate::Confirm>().get_open());
    assert!(h.app.confirmation.is_none());
    assert!(!h.app.ui.global::<crate::Preferences>().get_cache_busy());
}

#[test]
fn confirmation_cancel_button_dispatches_real_pointer_callback() {
    use slint::platform::{PointerEventButton, WindowEvent};
    let mut h = Harness::new();
    h.app.confirm(
        confirmation::Action::ClearCache,
        "清理缓存",
        "测试内容",
        "清理",
    );
    h.render();
    let position = slint::LogicalPosition::new(90.0, 116.0);
    h.window.dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    h.window.dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
    let message = h.messages.try_recv().unwrap();
    assert!(matches!(&message, Msg::Confirmed(false)));
    h.app.handle_message(message);
    assert!(h.app.confirmation.is_none());
}

#[test]
fn collection_buttons_follow_sidebar_slot_shift() {
    let mut h = Harness::new();
    h.app.page = Page::Collection;
    h.app.collection.items = vec![item("BV-collection")];
    for (position, play_key, detail_key) in [
        (MenuPosition::Top, "4", "5"),
        (MenuPosition::Left, "5", "6"),
        (MenuPosition::Right, "4", "5"),
    ] {
        h.app.settings.menu_position = position;
        h.app.requested_play = None;
        h.app.requested_detail = None;
        h.key(play_key);
        assert_eq!(h.app.requested_play.as_deref(), Some("BV-collection"));
        h.key(detail_key);
        assert_eq!(h.app.requested_detail.as_ref().unwrap().0, "BV-collection");
    }
}

#[test]
fn favorites_buttons_select_open_and_scroll_folders() {
    let mut h = Harness::new();
    h.app.page = Page::Favorites;
    h.app.favorites.folders = (0..6)
        .map(|id| model::FavoriteFolder {
            id,
            title: format!("收藏 {id}"),
            count: 2,
            private: id == 5,
            cover: String::new(),
        })
        .collect();
    h.key("RIGHT");
    h.key("DOWN");
    h.sync();
    assert_eq!(h.app.favorites.selection, 5);
    assert_eq!(h.app.favorites.first, 2);
    let rows = h.app.ui.global::<crate::Favorite>().get_folders();
    assert_eq!(rows.row_count(), 4);
    assert_eq!(rows.row_data(3).unwrap().visibility, "私密");
    h.key("4");
    assert_eq!(h.app.requested_folder.as_ref().unwrap().id, 5);
    h.key("DOWN");
    assert_eq!(h.app.favorites.selection, 5);
}

#[test]
fn following_navigation_skips_headers_and_opens_selected_author() {
    use following::FollowingEntry::{Header, User};
    let mut h = Harness::new();
    h.app.page = Page::Following;
    let user = |mid: &str| {
        User(model::Following {
            owner: model::Owner {
                mid: mid.into(),
                name: format!("作者 {mid}"),
            },
            face: String::new(),
            signature: String::new(),
        })
    };
    h.app.following.items = vec![
        Header {
            name: "分组一".into(),
            count: 1,
        },
        user("1"),
        Header {
            name: "分组二".into(),
            count: 2,
        },
        user("2"),
        user("3"),
    ];
    h.app.following.selection = 1;
    h.key("DOWN");
    assert_eq!(h.app.following.selection, 3);
    h.key("4");
    assert_eq!(h.app.requested_uploader.as_ref().unwrap().mid, "2");
    h.key("UP");
    h.key("UP");
    assert_eq!(h.app.following.selection, 1);
}

#[test]
fn mine_entries_request_their_corresponding_pages() {
    for selection in 0..4 {
        let mut h = Harness::new();
        h.app.tab = HomeTab::Mine;
        h.app.sel = selection;
        h.key("ENTER");
        assert_eq!(h.app.requested_following, selection == 0);
        assert_eq!(h.app.requested_home_list, (selection == 1).then_some(3));
        assert_eq!(h.app.requested_favorites, selection == 2);
        assert_eq!(h.app.requested_watch_later, selection == 3);
    }
}

#[test]
fn back_navigation_restores_selected_video_by_id_after_list_reordering() {
    let mut h = Harness::new();
    h.videos(3);
    h.app.sel = 1;
    h.app.list_scroll = 30.0;
    h.app.grid_scroll = 72.0;
    h.app.requested_about = true;
    h.app.navigate();
    assert_eq!(h.app.page, Page::About);
    h.app.lists[0].swap(1, 2);
    h.app.sel = 0;
    h.app.list_scroll = 0.0;
    h.app.grid_scroll = 0.0;
    h.key("8");
    h.key("8_UP");
    h.app.navigate();
    assert_eq!(h.app.page, Page::Home);
    assert_eq!(h.app.sel, 2);
    assert_eq!((h.app.list_scroll, h.app.grid_scroll), (30.0, 72.0));
    assert!(h.app.back_stack.is_empty());
}

#[test]
fn live_area_callback_opens_child_categories_and_back_restores_parent_list() {
    let mut h = Harness::new();
    h.app.tab = HomeTab::Live;
    h.app.live_list.areas = vec![
        model::LiveArea {
            name: "全部".into(),
            ..Default::default()
        },
        model::LiveArea {
            parent_id: 2,
            id: 0,
            name: "游戏".into(),
        },
        model::LiveArea {
            parent_id: 2,
            id: 21,
            name: "单机".into(),
        },
    ];
    h.key("5");
    assert!(h.app.ui.global::<crate::LiveAreas>().get_open());
    assert_eq!(
        h.app.ui.global::<crate::LiveAreas>().get_rows().row_count(),
        2
    );
    h.app
        .ui
        .global::<crate::LiveAreas>()
        .invoke_key("AREA_SELECT:1".into());
    h.dispatch_callbacks();
    assert_eq!(
        h.app.ui.global::<crate::LiveAreas>().get_title(),
        "选择分区 · 游戏"
    );
    h.key("8");
    h.key("8_UP");
    assert!(h.app.ui.global::<crate::LiveAreas>().get_open());
    assert_eq!(
        h.app.ui.global::<crate::LiveAreas>().get_title(),
        "选择直播分区"
    );
    h.key("8");
    h.key("8_UP");
    assert!(!h.app.ui.global::<crate::LiveAreas>().get_open());
}

#[test]
fn super_chat_scrolling_clamps_to_panel_bounds() {
    let mut h = Harness::new();
    h.app.page = Page::Player;
    h.app.ui.global::<crate::SuperChat>().set_open(true);
    h.app.ui.global::<crate::SuperChat>().set_content_h(300.0);
    h.app.ui.global::<crate::SuperChat>().set_viewport_h(100.0);
    h.key("UP");
    assert_eq!(h.app.ui.global::<crate::SuperChat>().get_scroll(), 0.0);
    h.key("DOWN");
    assert_eq!(h.app.ui.global::<crate::SuperChat>().get_scroll(), 22.0);
    h.key("PGDN");
    h.key("PGDN");
    assert_eq!(h.app.ui.global::<crate::SuperChat>().get_scroll(), 200.0);
    assert!(h.app.seek_pending.is_none());
}
