use crate::{HomeTab, MenuPosition, Page};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Sections,
    Select,
    SpeedTest,
    Refresh,
    Login,
    Help,
    Back,
    Works,
    Open,
    Author,
    Related,
    Play,
    Detail,
    Retry,
    TogglePlay,
    Like,
    Speed,
    Danmaku,
    SuperChat,
    LiveArea,
    Comments,
}

impl Action {
    pub fn label(self) -> &'static str {
        match self {
            Self::Sections => "选集",
            Self::Select => "选择",
            Self::SpeedTest => "测速",
            Self::Refresh => "刷新",
            Self::Login => "登录",
            Self::Help => "说明",
            Self::Back => "返回",
            Self::Works => "作品",
            Self::Open => "打开",
            Self::Author => "作者",
            Self::Related => "相关",
            Self::Play => "播放",
            Self::Detail => "详情",
            Self::Retry => "重试",
            Self::TogglePlay => "播放",
            Self::Like => "点赞",
            Self::Speed => "倍速",
            Self::Danmaku => "弹幕",
            Self::SuperChat => "醒目留言",
            Self::LiveArea => "分区",
            Self::Comments => "评论",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Sections => "p",
            Self::Select => "ENTER",
            Self::SpeedTest => "r",
            Self::Refresh | Self::Retry => "r",
            Self::Login => "l",
            Self::Help => "h",
            Self::Back => "ESC",
            Self::Works => "u",
            Self::Author => "u",
            Self::Related => "RELATED",
            Self::Play | Self::Open => "ENTER",
            Self::Detail | Self::TogglePlay => "SPACE",
            Self::Like => "l",
            Self::Speed => "b",
            Self::Danmaku => "d",
            Self::SuperChat => "SUPER_CHAT",
            Self::LiveArea => "LIVE_AREA",
            Self::Comments => "r",
        }
    }
}

// 五个槽位依次对应实体 4–8 键，首页菜单使用全部槽位
pub fn actions(
    page: Page,
    tab: HomeTab,
    mine_detail: i32,
    help: bool,
    logged: bool,
    position: MenuPosition,
    retry: bool,
) -> [Option<Action>; 5] {
    use Action::*;
    let mut slots = [None; 5];
    let first = usize::from(page != Page::Home && position == MenuPosition::Left);
    let last = 4;
    if help {
        slots[last] = Some(Back);
        return slots;
    }
    match page {
        Page::Home => {
            if tab != HomeTab::Settings {
                slots[first] = Some(Refresh);
                if !logged {
                    slots[first + 1] = Some(Login);
                }
            }
            if tab.is_video() || mine_detail >= 0 {
                slots[2] = Some(Play);
                if tab != HomeTab::Live {
                    slots[3] = Some(Detail);
                }
            }
            if tab == HomeTab::Live {
                slots[1] = Some(LiveArea);
                if !logged {
                    slots[3] = Some(Login);
                }
            }
            slots[4] = Some(if mine_detail >= 0 { Back } else { Help });
        }
        Page::Player => {
            return [
                Some(TogglePlay),
                Some(Like),
                Some(Speed),
                Some(Danmaku),
                Some(Comments),
            ]
        }
        Page::Detail => return [None, Some(Play), Some(Author), Some(Related), Some(Back)],
        Page::Login => {
            slots[first] = Some(Refresh);
            slots[last] = Some(Back);
        }
        Page::About => slots[last] = Some(Back),
        Page::Favorites => {
            slots[first] = Some(if retry { Retry } else { Open });
            slots[last] = Some(Back);
        }
        Page::Following => {
            slots[first] = Some(if retry { Retry } else { Works });
            slots[last] = Some(Back);
        }
        Page::Comments => {
            if retry {
                slots[first] = Some(Retry);
            }
            slots[last] = Some(Back);
        }
        Page::Collection => {
            slots[first] = Some(if retry { Retry } else { Play });
            slots[first + 1] = Some(Detail);
            slots[last] = Some(Back);
        }
    }
    slots
}

#[cfg(test)]
mod tests;
