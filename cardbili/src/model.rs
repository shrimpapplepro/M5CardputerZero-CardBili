#[derive(Clone, Default)]
pub struct Item {
    pub title: String,
    pub meta: String,
    pub bvid: String,
    pub pic: String,
    pub duration: String,
    pub views: Option<String>,
    pub likes: Option<String>,
    pub danmaku: Option<String>,
    pub progress: i64,
    pub view_at: i64,
    pub date: Option<String>,
    pub duration_secs: f64,
}

#[derive(Clone, Default)]
pub struct Owner {
    pub mid: String,
    pub name: String,
}

#[derive(Clone)]
pub enum VideoSource {
    Author(Owner),
    Related(String),
    Favorites(FavoriteFolder),
    WatchLater,
}

impl Default for VideoSource {
    fn default() -> Self {
        Self::Author(Owner::default())
    }
}

pub struct AuthorProfile {
    pub name: String,
    pub face: String,
    pub level: i64,
    pub followers: i64,
}

pub struct VideoPage {
    pub items: Vec<Item>,
    pub has_more: bool,
    pub total: Option<u64>,
}

pub struct LivePage {
    pub following: Vec<Item>,
    pub popular: VideoPage,
}

#[derive(Clone, Default)]
pub struct PlayInfo {
    pub aid: i64,
    pub bvid: String,
    pub owner: Owner,
    pub title: String,
    pub video_url: String,
    pub audio_url: String,
    pub duration: f64,
    pub cid: i64,
    pub parts: Vec<VideoPart>,
    pub live: bool,
}

// 一级分区的 id 为 0，parent_id 与 id 同为 0 表示全部直播
#[derive(Clone, Default)]
pub struct LiveArea {
    pub parent_id: u64,
    pub id: u64,
    pub name: String,
}

#[derive(Clone, Default)]
pub struct LiveInfo {
    pub room_id: u64,
    pub title: String,
    pub uname: String,
    pub online: String,
    pub play_url: String,
    pub started_at: i64,
}

#[derive(Clone, Default)]
pub struct Account {
    pub logged: bool,
    pub name: String,
    pub mid: String,
    pub level: i64,
    pub vip: String,
    pub face: String,
}

#[derive(Clone, Default)]
pub struct CommentEmote {
    pub name: String,
    pub url: String,
    // 展示边长：小表情 12，其它类型 24
    pub size: u32,
}

#[derive(Clone, Default)]
pub struct Comment {
    pub user: String,
    pub text: String,
    pub created_at: i64,
    pub likes: i64,
    pub emotes: Vec<CommentEmote>,
}

#[derive(Clone, Default)]
pub struct Detail {
    pub aid: i64,
    pub cid: i64,
    pub parts: Vec<VideoPart>,
    pub title: String,
    pub pic: String,
    pub duration: f64,
    pub owner: Owner,
    pub views: i64,
    pub likes: i64,
    pub coins: i64,
    pub favorites: i64,
    pub danmaku: i64,
    pub desc: String,
}

pub fn mmss(sec: f64) -> String {
    let s = if sec.is_finite() && sec > 0.0 {
        sec as i64
    } else {
        0
    };
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let ss = s % 60;
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, ss)
    } else {
        format!("{}:{:02}", m, ss)
    }
}

pub fn dur_of(v: &serde_json::Value) -> f64 {
    v.as_f64()
        .or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok()))
        .unwrap_or(0.0)
}

pub fn fmt_num(n: i64) -> String {
    if n >= 100_000_000 {
        format!("{:.1}亿", n as f64 / 1e8)
    } else if n >= 10_000 {
        format!("{:.1}万", n as f64 / 1e4)
    } else {
        n.to_string()
    }
}

pub fn watch_time(ts: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    watch_time_at(ts, now)
}

fn watch_time_at(ts: i64, now: i64) -> String {
    if ts <= 0 {
        return String::new();
    }
    let local = ts + 8 * 3600;
    let day = local.div_euclid(86400);
    let sod = local.rem_euclid(86400);
    let hh = sod / 3600;
    let mm = (sod % 3600) / 60;
    let today = (now + 8 * 3600).div_euclid(86400);
    match today - day {
        0 => format!("今天 {:02}:{:02}", hh, mm),
        1 => format!("昨天 {:02}:{:02}", hh, mm),
        _ => {
            let (_, month, date) = civil_from_days(day);
            format!("{:02}-{:02} {:02}:{:02}", month, date, hh, mm)
        }
    }
}
pub fn date_of(ts: i64) -> String {
    if ts <= 0 {
        return String::new();
    }
    let day = (ts + 8 * 3600).div_euclid(86400);
    let (year, month, date) = civil_from_days(day);
    format!("{}-{:02}-{:02}", year, month, date)
}
pub fn history_day_label(ts: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    history_day_label_at(ts, now)
}

fn history_day_label_at(ts: i64, now: i64) -> String {
    if ts <= 0 {
        return String::new();
    }
    let local = ts + 8 * 3600;
    let day = local.div_euclid(86400);
    let today = (now + 8 * 3600).div_euclid(86400);
    match today - day {
        0 => "今天".into(),
        1 => "昨天".into(),
        _ => {
            let (year, month, date) = civil_from_days(day);
            format!("{}年{}月{}日", year, month, date)
        }
    }
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    (y + if m <= 2 { 1 } else { 0 }, m as u32, d as u32)
}

#[derive(Clone, Copy)]
pub enum VideoAction {
    Like(bool),
    Triple,
}

pub struct ReactionResult {
    pub triple_complete: bool,
    pub liked: bool,
    pub message: String,
}

pub struct CommentPage {
    pub items: Vec<Comment>,
    pub has_more: bool,
}

pub struct DynamicPage {
    pub items: Vec<Item>,
    pub offset: String,
    pub has_more: bool,
}

#[derive(Clone)]
pub struct Following {
    pub owner: Owner,
    pub face: String,
    pub signature: String,
}

pub struct FollowingPage {
    pub items: Vec<Following>,
    pub has_more: bool,
}

#[derive(Clone)]
pub struct FavoriteFolder {
    pub id: i64,
    pub title: String,
    pub count: u64,
    pub private: bool,
    pub cover: String,
}

#[derive(Clone, Default)]
pub struct HistoryCursor {
    pub max: i64,
    pub view_at: i64,
    pub business: String,
}

pub struct HistoryPage {
    pub items: Vec<Item>,
    pub cursor: HistoryCursor,
    pub has_more: bool,
}

#[derive(Clone)]
pub struct VideoPart {
    pub cid: i64,
    pub title: String,
    pub duration: f64,
}

#[derive(Clone)]
pub struct Chapter {
    pub title: String,
    pub start: f64,
}

#[cfg(test)]
mod tests;
