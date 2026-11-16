use super::*;

#[test]
fn synchronized_menu_tracks_login_and_help_changes() {
    let mut h = Harness::new();
    h.sync();
    assert_eq!(h.labels(), ["刷新", "登录", "播放", "详情", "说明"]);
    h.app.cookies = Some("DedeUserID=1".into());
    h.sync();
    assert_eq!(h.labels(), ["刷新", "", "播放", "详情", "说明"]);
    h.key("8");
    h.sync();
    assert_eq!(h.labels(), ["", "", "", "", "返回"]);
    h.key("8");
    assert!(h.app.help);
    h.key("8_UP");
    h.sync();
    assert!(!h.app.help);
    assert_eq!(h.labels()[4], "说明");
}

#[test]
fn overlays_replace_background_menu_and_restore_it_on_close() {
    let mut h = Harness::new();
    h.app.ui.global::<crate::Cdn>().set_open(true);
    h.sync();
    assert_eq!(h.labels(), ["测速", "选择", "", "", "返回"]);
    h.app.ui.global::<crate::LiveAreas>().set_open(true);
    h.sync();
    assert_eq!(h.labels(), ["刷新", "选择", "", "", "返回"]);
    h.key("8");
    h.key("8_UP");
    h.sync();
    assert_eq!(h.labels(), ["测速", "选择", "", "", "返回"]);
    h.key("8");
    h.key("8_UP");
    h.sync();
    assert_eq!(h.labels(), ["刷新", "登录", "播放", "详情", "说明"]);
}

#[test]
fn live_player_menu_tracks_super_chat_availability_and_popup() {
    let mut h = Harness::new();
    h.app.page = Page::Player;
    h.app.live = Some(model::LiveInfo {
        room_id: 42,
        ..Default::default()
    });
    h.sync();
    assert_eq!(h.labels(), ["播放", "刷新", "", "弹幕", ""]);
    h.app.ui.global::<crate::SuperChat>().set_available(true);
    h.sync();
    assert_eq!(h.labels(), ["播放", "刷新", "醒目留言", "弹幕", ""]);
    h.app.ui.global::<crate::SuperChat>().set_open(true);
    h.sync();
    assert_eq!(h.labels(), ["", "", "", "", "返回"]);
    h.key("8");
    assert!(!h.app.ui.global::<crate::SuperChat>().get_open());
    assert!(!h.app.go_back);
}

#[test]
fn sections_menu_routes_play_and_return_to_the_panel() {
    let mut h = Harness::new();
    h.app.page = Page::Detail;
    h.app.sections.open = true;
    h.sync();
    assert_eq!(h.labels(), ["", "播放", "", "", "返回"]);
    h.key("8");
    h.key("8_UP");
    assert!(!h.app.sections.open);
    assert!(!h.app.go_back);
}

#[test]
fn detail_menu_shows_sections_and_hides_redundant_author_link() {
    let mut h = Harness::new();
    h.app.page = Page::Detail;
    h.app.sections.chapters = vec![model::Chapter {
        title: "第一章".into(),
        start: 0.0,
    }];
    h.app
        .back_stack
        .push(BackTarget::Collection(CollectionState::default()));
    h.sync();
    assert_eq!(h.labels(), ["选集", "播放", "", "相关", "返回"]);
    h.key("4");
    assert!(h.app.sections.open);
}

#[test]
fn retry_menu_reflects_each_pages_own_error() {
    let mut h = Harness::new();
    h.app.settings.menu_position = MenuPosition::Left;
    for page in [
        Page::Comments,
        Page::Following,
        Page::Favorites,
        Page::Collection,
    ] {
        h.app.page = page;
        h.app.comment_page.error = "failed".into();
        h.app.following.error = "failed".into();
        h.app.favorites.error = "failed".into();
        h.app.collection.error = "failed".into();
        h.sync();
        assert_eq!(h.labels()[0], "");
        assert_eq!(h.labels()[1], "重试", "page={page:?}");
        assert_eq!(h.labels()[4], "返回");
    }
}
