mod about;
mod account;
mod cache;
mod cdn;
mod collection;
mod comments;
mod components;
mod confirmation;
mod detail;
mod favorites;
mod following;
mod history;
mod home;
mod input;
mod login;
mod messages;
mod navigation;
mod platform;
mod playback;
mod scroll;
mod sections;
mod state;
mod view;

use crate::player::{HEIGHT, WIDTH};
use crate::settings::Settings;
use crate::{
    danmaku, menu, model, player, tasks, util, AppWindow, Cmt, HomeTab, MenuPosition, Msg, Page,
    VideoLayout, VideoRow,
};
use collection::CollectionState;
use components::*;
use detail::DetailDisplay;
use detail::{show_detail, show_detail_loading, sync_thumb};
use navigation::BackTarget;
use slint::{ComponentHandle, ModelRc, VecModel};
use state::App;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

pub fn run(settings: Settings) {
    crate::cache::init();
    let (tx, rx) = mpsc::channel();
    let (ktx, krx) = mpsc::channel();
    let mut display = platform::Display::new(ktx.clone());
    let ui = AppWindow::new().expect("ui");
    ui.show().expect("show");
    connect_ui(&ui, tx.clone(), ktx);
    let mut app = App::new(ui, tx, settings);
    #[cfg(feature = "desktop")]
    if !crate::diagnostics::ffmpeg_available() {
        #[cfg(windows)]
        app.confirm(
            confirmation::Action::FfmpegInstall,
            "未找到 FFmpeg",
            "播放需要 FFmpeg，是否通过 winget 自动安装？",
            "安装",
        );
        #[cfg(target_os = "macos")]
        app.confirm(
            confirmation::Action::FfmpegNotice,
            "未找到 FFmpeg",
            "播放需要 FFmpeg，请先执行 brew install ffmpeg",
            "知道了",
        );
        #[cfg(all(not(windows), not(target_os = "macos")))]
        app.confirm(
            confirmation::Action::FfmpegNotice,
            "未找到 FFmpeg",
            "播放需要 FFmpeg，请先通过系统包管理器安装",
            "知道了",
        );
    }
    let mut pending = None;
    while !app.quit {
        let tick_end = Instant::now() + Duration::from_millis(16);
        if app
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            break;
        }
        app.settings_changed = false;
        app.go_back = false;
        app.tick();
        if app.quit {
            return;
        }
        if let Some(message) = pending.take() {
            app.handle_message(message);
        }
        while let Ok(message) = rx.try_recv() {
            app.handle_message(message);
        }
        display.poll_keys(&app.ui);
        while let Ok(key) = krx.try_recv() {
            app.handle_key(key);
            if app.quit {
                return;
            }
        }
        app.navigate();
        app.sync_view();
        if !display.render() {
            break;
        }
        pending = rx
            .recv_timeout(tick_end.saturating_duration_since(Instant::now()))
            .ok();
    }
    app.close_cdn();
    app.report_history(true, false);
    if let Some(mut player) = app.player.take() {
        player.stop();
    }
    app.history.flush();
}

fn connect_ui(ui: &AppWindow, tx: mpsc::Sender<Msg>, ktx: mpsc::Sender<String>) {
    let area_keys = ktx.clone();
    ui.global::<crate::LiveAreas>().on_key(move |key| {
        let _ = area_keys.send(key.to_string());
    });
    ui.global::<crate::Player>().on_key(move |key| {
        let _ = ktx.send(key.to_string());
    });
    let confirm_tx = tx.clone();
    ui.global::<crate::Confirm>().on_result(move |accepted| {
        let _ = confirm_tx.send(Msg::Confirmed(accepted));
    });
}

#[cfg(test)]
mod tests;
