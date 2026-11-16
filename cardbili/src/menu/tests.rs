use super::*;

fn labels(slots: [Option<Action>; 5]) -> [&'static str; 5] {
    slots.map(|action| action.map(Action::label).unwrap_or(""))
}

const PAGES: [Page; 9] = [
    Page::Home,
    Page::Player,
    Page::Detail,
    Page::Login,
    Page::About,
    Page::Favorites,
    Page::Following,
    Page::Comments,
    Page::Collection,
];
const POSITIONS: [MenuPosition; 3] = [MenuPosition::Top, MenuPosition::Left, MenuPosition::Right];

#[test]
fn home_menu_covers_every_tab_and_login_state() {
    for position in POSITIONS {
        for (tab, anonymous, logged) in [
            (
                HomeTab::Recommended,
                ["刷新", "登录", "播放", "详情", "说明"],
                ["刷新", "", "播放", "详情", "说明"],
            ),
            (
                HomeTab::Popular,
                ["刷新", "登录", "播放", "详情", "说明"],
                ["刷新", "", "播放", "详情", "说明"],
            ),
            (
                HomeTab::Dynamic,
                ["刷新", "登录", "播放", "详情", "说明"],
                ["刷新", "", "播放", "详情", "说明"],
            ),
            (
                HomeTab::Live,
                ["刷新", "分区", "播放", "登录", "说明"],
                ["刷新", "分区", "播放", "", "说明"],
            ),
            (
                HomeTab::Mine,
                ["刷新", "登录", "", "", "说明"],
                ["刷新", "", "", "", "说明"],
            ),
            (
                HomeTab::Settings,
                ["", "", "", "", "说明"],
                ["", "", "", "", "说明"],
            ),
        ] {
            for (is_logged, expected) in [(false, anonymous), (true, logged)] {
                assert_eq!(
                    labels(actions(
                        Page::Home,
                        tab,
                        -1,
                        false,
                        is_logged,
                        position,
                        false
                    )),
                    expected,
                    "tab={tab:?}, logged={is_logged}, position={position:?}"
                );
            }
        }
    }
}

#[test]
fn history_menu_has_play_detail_and_return() {
    assert_eq!(
        labels(actions(
            Page::Home,
            HomeTab::Mine,
            3,
            false,
            true,
            MenuPosition::Left,
            false
        )),
        ["刷新", "", "播放", "详情", "返回"]
    );
}

#[test]
fn help_overrides_all_page_menus() {
    for page in PAGES {
        for position in POSITIONS {
            assert_eq!(
                labels(actions(
                    page,
                    HomeTab::Recommended,
                    -1,
                    true,
                    false,
                    position,
                    true
                )),
                ["", "", "", "", "返回"]
            );
        }
    }
}

#[test]
fn player_and_detail_keep_fixed_physical_key_slots() {
    for position in POSITIONS {
        assert_eq!(
            labels(actions(
                Page::Player,
                HomeTab::Recommended,
                -1,
                false,
                true,
                position,
                false
            )),
            ["播放", "点赞", "倍速", "弹幕", "评论"]
        );
        assert_eq!(
            labels(actions(
                Page::Detail,
                HomeTab::Recommended,
                -1,
                false,
                true,
                position,
                false
            )),
            ["", "播放", "作者", "相关", "返回"]
        );
    }
}

#[test]
fn secondary_page_actions_leave_left_sidebar_slot_free() {
    for position in POSITIONS {
        for (page, normal, retry) in [
            (Page::Login, "刷新", "刷新"),
            (Page::Favorites, "打开", "重试"),
            (Page::Following, "作品", "重试"),
            (Page::Comments, "", "重试"),
            (Page::Collection, "播放", "重试"),
            (Page::About, "", ""),
        ] {
            for (failed, first_label) in [(false, normal), (true, retry)] {
                let mut expected = ["", "", "", "", "返回"];
                let first = usize::from(position == MenuPosition::Left);
                expected[first] = first_label;
                if page == Page::Collection {
                    expected[first + 1] = "详情";
                }
                assert_eq!(
                    labels(actions(
                        page,
                        HomeTab::Recommended,
                        -1,
                        false,
                        true,
                        position,
                        failed
                    )),
                    expected,
                    "page={page:?}, position={position:?}, retry={failed}"
                );
            }
        }
    }
}

#[test]
fn every_menu_action_has_the_expected_dispatch_key() {
    for (action, expected) in [
        (Action::Sections, "p"),
        (Action::Select, "ENTER"),
        (Action::SpeedTest, "r"),
        (Action::Refresh, "r"),
        (Action::Login, "l"),
        (Action::Help, "h"),
        (Action::Back, "ESC"),
        (Action::Works, "u"),
        (Action::Open, "ENTER"),
        (Action::Author, "u"),
        (Action::Related, "RELATED"),
        (Action::Play, "ENTER"),
        (Action::Detail, "SPACE"),
        (Action::Retry, "r"),
        (Action::TogglePlay, "SPACE"),
        (Action::Like, "l"),
        (Action::Speed, "b"),
        (Action::Danmaku, "d"),
        (Action::SuperChat, "SUPER_CHAT"),
        (Action::LiveArea, "LIVE_AREA"),
        (Action::Comments, "r"),
    ] {
        assert_eq!(action.key(), expected, "action={}", action.label());
    }
}
