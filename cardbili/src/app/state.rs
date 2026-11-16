use super::*;
use slint::ComponentHandle as _;

pub(super) struct App {
    pub(super) volume_queue: mpsc::SyncSender<i32>,
    pub(super) history: history::History,
    pub(super) controls: playback::Controls,
    pub(super) comment_page: comments::Comments,
    pub(super) sections: sections::Sections,
    pub(super) requested_cid: Option<i64>,
    pub(super) play_start: f64,
    pub(super) comments_request: u64,
    pub(super) requested_comments: Option<i64>,
    pub(super) ui: AppWindow,
    pub(super) tx: mpsc::Sender<Msg>,
    pub(super) cookies: Option<String>,
    pub(super) mid: String,
    pub(super) ph: slint::Image,
    pub(super) page: Page,
    pub(super) tab: HomeTab,
    pub(super) settings: Settings,
    pub(super) cache_checked: Option<Instant>,
    pub(super) confirmation: Option<confirmation::Action>,
    pub(super) cdn_request: std::sync::Arc<std::sync::atomic::AtomicU64>,
    pub(super) view: VideoLayout,
    pub(super) help: bool,
    pub(super) sel: i32,
    pub(super) lists: Vec<Vec<model::Item>>,
    pub(super) covers: Vec<Vec<Option<slint::Image>>>,
    pub(super) covers_started: [bool; 6],
    pub(super) loaded: [bool; 6],
    pub(super) pages: [usize; 6],
    pub(super) first_page_sizes: [usize; 6],
    pub(super) has_more: [bool; 6],
    pub(super) loading_more: [bool; 6],
    pub(super) mine_detail: i32,
    pub(super) acct: Option<model::Account>,
    pub(super) avatar_img: Option<slint::Image>,
    pub(super) acct_loaded: bool,
    pub(super) account_dirty: bool,
    pub(super) home_dirty: bool,
    pub(super) home_view: Option<(usize, VideoLayout, Page, usize, MenuPosition)>,
    pub(super) menu_view: Option<[Option<menu::Action>; 5]>,
    pub(super) player: Option<player::Player>,
    pub(super) playback_display: playback::PlaybackDisplay,
    pub(super) frames_in_seg: u64,
    pub(super) gen: u64,
    pub(super) danmaku: Option<danmaku::Engine>,
    pub(super) danmaku_view: DanmakuView,
    pub(super) status: String,
    pub(super) status_detail: String,
    pub(super) loading: bool,
    pub(super) detail_bvid: String,
    pub(super) detail_request: u64,
    pub(super) detail_display: Option<DetailDisplay>,
    pub(super) play_request: u64,
    pub(super) current_play: Option<model::PlayInfo>,
    pub(super) requested_play: Option<String>,
    pub(super) requested_live: Option<u64>,
    pub(super) live_list: home::LiveState,
    pub(super) live: Option<model::LiveInfo>,
    pub(super) live_recovery: playback::LiveRecovery,
    pub(super) live_danmaku: Option<crate::live_danmaku::Connection>,
    pub(super) live_engine: danmaku::LiveEngine,
    pub(super) super_chat: crate::super_chat::State,
    pub(super) requested_uploader: Option<model::Owner>,
    pub(super) requested_related: Option<String>,
    pub(super) requested_login: bool,
    pub(super) login_request: u64,
    pub(super) login_uploader: Option<model::Owner>,
    pub(super) history_cursor: model::HistoryCursor,
    pub(super) history_request: u64,
    pub(super) dynamic_offset: String,
    pub(super) dynamic_request: u64,
    pub(super) following: following::FollowingState,
    pub(super) following_request: u64,
    pub(super) requested_following: bool,
    pub(super) login_following: bool,
    pub(super) favorites: favorites::Favorites,
    pub(super) requested_favorites: bool,
    pub(super) requested_folder: Option<model::FavoriteFolder>,
    pub(super) requested_watch_later: bool,
    pub(super) requested_home_list: Option<usize>,
    pub(super) requested_about: bool,
    pub(super) login_favorites: bool,
    pub(super) collection: CollectionState,
    pub(super) collection_request: u64,
    pub(super) collection_dirty: bool,
    pub(super) back_stack: Vec<BackTarget>,
    pub(super) row_tops: Vec<f32>,
    pub(super) list_scroll: f64,
    pub(super) grid_scroll: f64,
    pub(super) scroll_context: Option<(i32, VideoLayout, Page, MenuPosition)>,
    pub(super) about_scroll: f64,
    pub(super) requested_detail: Option<(String, String)>,
    pub(super) detail_scroll: f64,
    pub(super) detail_maxv: f64,
    pub(super) marq_t: f64,
    pub(super) last_tick: Instant,
    pub(super) deadline: Option<Instant>,
    pub(super) quit: bool,
    pub(super) seek_pending: Option<f64>,
    pub(super) seek_deadline: Option<Instant>,
    pub(super) esc_press: Option<Instant>,
    pub(super) bar_until: Option<Instant>,
    pub(super) last_input: Instant,
    pub(super) direction_hold: Option<input::DirectionHold>,
    pub(super) return_hold: Option<(usize, Instant)>,
    pub(super) back_taps: (u8, Option<Instant>),
    pub(super) back_hint: bool,
    pub(super) return_home: bool,
    pub(super) latest: Option<(u64, player::VideoFrame)>,
    pub(super) settings_changed: bool,
    pub(super) go_back: bool,
}

impl App {
    pub(super) fn new(ui: AppWindow, tx: mpsc::Sender<Msg>, settings: Settings) -> Self {
        let cookies = crate::net::session::restore();
        crate::diagnostics::write(format_args!(
            "[登录] 已读取登录态 登录={}",
            cookies.is_some()
        ));
        let login = ui.global::<crate::Login>();
        let mut page = Page::Home;
        let tab = std::env::var("BILI_TAB")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .map(HomeTab::from_index)
            .unwrap_or(HomeTab::Recommended);
        let mut view = std::env::var("BILI_VIEW")
            .ok()
            .and_then(|s| s.parse::<i32>().ok())
            .map(VideoLayout::from_index)
            .unwrap_or(settings.list_view);
        let help: bool = std::env::var("BILI_HELP").is_ok();
        let sel: i32 = std::env::var("BILI_SEL")
            .ok()
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(0);
        let mut status = String::from("加载中…");
        let loading = tab.is_video();

        // 详情页
        let mut requested_play: Option<String> = None;
        let mut login_request = 0u64;
        let mut requested_detail: Option<(String, String)> = None;

        let force0 = std::env::var("BILI_FORCE").is_ok();
        if tab == HomeTab::Live {
            tasks::load_live_list(
                0,
                model::LiveArea::default(),
                1,
                cookies.clone(),
                tx.clone(),
                force0,
            );
        } else if tab.is_video() {
            tasks::load_list(tab.index(), cookies.clone(), tx.clone(), force0);
        }

        if let Ok(b) = std::env::var("BILI_AUTOPLAY") {
            if !b.is_empty() {
                status = "正在加载视频…".into();
                requested_play = Some(b);
            }
        }
        let mut requested_live: Option<u64> = None;
        if let Ok(v) = std::env::var("BILI_LIVE") {
            requested_live = v.parse::<u64>().ok();
            if requested_live.is_some() {
                status = "正在加载直播…".into();
            }
        }
        if let Ok(b) = std::env::var("BILI_DETAIL") {
            if !b.is_empty() {
                requested_detail = Some((b, String::new()));
            }
        }
        if std::env::var("BILI_ABOUT").is_ok() {
            page = Page::About;
        }
        if std::env::var("BILI_TEST_LOGIN").is_ok() {
            page = Page::Login;
            view = VideoLayout::List;
            login.set_has_qr(false);
            login.set_qr_status("正在生成二维码…".into());
            login_request += 1;
            tasks::start_login(login_request, tx.clone());
        }
        let deadline = std::env::var("BILI_SECONDS")
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .map(|s| Instant::now() + Duration::from_secs_f64(s));

        let volume_queue = player::volume::worker(tx.clone());
        let mut app = Self::with_state(ui, tx, settings, cookies, volume_queue);
        app.page = page;
        app.tab = tab;
        app.view = view;
        app.help = help;
        app.sel = sel;
        app.status = status;
        app.loading = loading;
        app.requested_play = requested_play;
        app.requested_live = requested_live;
        app.requested_detail = requested_detail;
        app.login_request = login_request;
        app.deadline = deadline;
        app.play_start = std::env::var("BILI_START")
            .ok()
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| value.is_finite() && *value >= 0.0)
            .unwrap_or(0.0);
        app
    }

    pub(super) fn with_state(
        ui: AppWindow,
        tx: mpsc::Sender<Msg>,
        settings: Settings,
        cookies: Option<String>,
        volume_queue: mpsc::SyncSender<i32>,
    ) -> Self {
        crate::emoji::install(&ui);
        ui.global::<crate::Preferences>()
            .set_color_emoji(settings.color_emoji);
        let meta = ui.global::<crate::Meta>();
        meta.set_version(format!("v{}", env!("CARGO_PKG_VERSION")).into());
        meta.set_about_text("作者  Jad\n\n项目地址\ngithub.com/journey-ad/M5CardputerZero-CardBili\n\n字体\nFusion Pixel Font\ngithub.com/TakWolf/fusion-pixel-font\n\nMona Font\ngithub.com/MonadABXY/mona-font\n\n接口参考\nPiliPlus\ngithub.com/bggRGjQaUbCoE/PiliPlus\n\n图标\nPixelarticons\ngithub.com/halfmage/pixelarticons\n\n".into());

        let mid = cookies.as_deref().map(util::parse_mid).unwrap_or_default();
        let ph = util::make_placeholder();
        let view = settings.list_view;
        let danmaku_view = DanmakuView::new(&ui, settings.color_emoji);
        let last_input = Instant::now();
        Self {
            volume_queue,
            history: history::History::new(settings.record_history),
            controls: playback::Controls::default(),
            playback_display: playback::PlaybackDisplay::default(),
            comment_page: comments::Comments::default(),
            sections: sections::Sections::default(),
            requested_cid: None,
            play_start: 0.0,
            comments_request: 0,
            requested_comments: None,
            ui,
            tx,
            cookies,
            mid,
            ph,
            page: Page::Home,
            tab: HomeTab::Recommended,
            settings,
            cache_checked: None,
            confirmation: None,
            cdn_request: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            view,
            help: false,
            sel: 0,
            lists: vec![Vec::new(); 6],
            covers: vec![Vec::new(); 6],
            covers_started: [false; 6],
            loaded: [false; 6],
            pages: [1; 6],
            first_page_sizes: [0; 6],
            has_more: [true; 6],
            loading_more: [false; 6],
            mine_detail: -1,
            acct: None,
            avatar_img: None,
            acct_loaded: false,
            account_dirty: true,
            home_dirty: true,
            home_view: None,
            menu_view: None,
            player: None,
            frames_in_seg: 0,
            gen: 1,
            danmaku: None,
            danmaku_view,
            status: "加载中…".into(),
            status_detail: String::new(),
            loading: true,
            detail_bvid: String::new(),
            detail_request: 0,
            detail_display: None,
            play_request: 0,
            current_play: None,
            requested_play: None,
            requested_live: None,
            live_list: home::LiveState::default(),
            live: None,
            live_recovery: playback::LiveRecovery::default(),
            live_danmaku: None,
            live_engine: danmaku::LiveEngine::default(),
            super_chat: crate::super_chat::State::default(),
            requested_uploader: None,
            requested_related: None,
            requested_login: false,
            login_request: 0,
            login_uploader: None,
            history_cursor: model::HistoryCursor::default(),
            history_request: 0,
            dynamic_offset: String::new(),
            dynamic_request: 0,
            following: following::FollowingState::default(),
            following_request: 0,
            requested_following: false,
            login_following: false,
            favorites: favorites::Favorites::default(),
            requested_favorites: false,
            requested_folder: None,
            requested_watch_later: false,
            requested_home_list: None,
            requested_about: false,
            login_favorites: false,
            collection: CollectionState::default(),
            collection_request: 0,
            collection_dirty: false,
            back_stack: Vec::new(),
            row_tops: Vec::new(),
            list_scroll: 0.0,
            grid_scroll: 0.0,
            scroll_context: None,
            about_scroll: 0.0,
            requested_detail: None,
            detail_scroll: 0.0,
            detail_maxv: 0.0,
            marq_t: 0.0,
            // 跑马灯
            last_tick: last_input,
            deadline: None,
            quit: false,
            seek_pending: None,
            seek_deadline: None,
            esc_press: None,
            bar_until: None,
            last_input,
            direction_hold: None,
            return_hold: None,
            back_taps: (0, None),
            back_hint: false,
            return_home: false,
            latest: None,
            settings_changed: false,
            go_back: false,
        }
    }
}
