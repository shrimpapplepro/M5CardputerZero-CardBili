mod interaction;
mod layout;
mod menus;

use super::*;
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Platform, WindowAdapter};
use slint::Model;
use std::cell::RefCell;
use std::rc::Rc;

thread_local! {
    static LAST_WINDOW: RefCell<Option<Rc<MinimalSoftwareWindow>>> = const { RefCell::new(None) };
    static PLATFORM: () = slint::platform::set_platform(Box::new(TestPlatform)).unwrap();
}

struct TestPlatform;

impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        crate::fonts::register(window.as_ref());
        window.set_size(slint::PhysicalSize::new(320, 170));
        LAST_WINDOW.with(|last| *last.borrow_mut() = Some(window.clone()));
        Ok(window)
    }
}

struct Harness {
    app: App,
    window: Rc<MinimalSoftwareWindow>,
    messages: mpsc::Receiver<Msg>,
    keys: mpsc::Receiver<String>,
    _volume: mpsc::Receiver<i32>,
}

impl Harness {
    fn new() -> Self {
        PLATFORM.with(|_| {});
        let ui = AppWindow::new().unwrap();
        let window = LAST_WINDOW.with(|last| last.borrow().as_ref().unwrap().clone());
        let (tx, messages) = mpsc::channel();
        let (key_tx, keys) = mpsc::channel();
        super::connect_ui(&ui, tx.clone(), key_tx);
        let (volume, volume_rx) = mpsc::sync_channel(8);
        let mut app = App::with_state(ui, tx, Settings::default(), None, volume);
        app.loaded = [true; 6];
        app.has_more = [false; 6];
        app.acct_loaded = true;
        app.favorites.loaded = true;
        app.loading = false;
        Self {
            app,
            window,
            messages,
            keys,
            _volume: volume_rx,
        }
    }

    fn key(&mut self, key: &str) {
        self.app.handle_key(key.into());
    }

    fn sync(&mut self) {
        self.app.cache_checked = Some(Instant::now());
        self.app.sync_view();
    }

    fn dispatch_callbacks(&mut self) {
        while let Ok(key) = self.keys.try_recv() {
            self.app.handle_key(key);
        }
        while let Ok(message) = self.messages.try_recv() {
            self.app.handle_message(message);
        }
    }

    fn render(&self) -> Vec<slint::Rgb8Pixel> {
        self.app.ui.show().unwrap();
        self.window.request_redraw();
        let mut pixels = vec![slint::Rgb8Pixel::default(); 320 * 170];
        assert!(self.window.draw_if_needed(|renderer| {
            renderer.render(&mut pixels, 320);
        }));
        pixels
    }

    fn labels(&self) -> Vec<String> {
        self.app
            .ui
            .global::<crate::Menu>()
            .get_labels()
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    fn videos(&mut self, count: usize) {
        let source = self.app.tab.index();
        self.app.lists[source] = (0..count).map(|i| item(&format!("BV{i}"))).collect();
        self.app.covers[source] = vec![None; count];
        self.app.home_dirty = true;
    }
}

fn item(id: &str) -> model::Item {
    model::Item {
        bvid: id.into(),
        title: format!("视频 {id}"),
        views: Some("100".into()),
        ..Default::default()
    }
}
