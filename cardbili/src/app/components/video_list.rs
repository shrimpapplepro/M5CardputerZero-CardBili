use super::super::*;
use slint::ComponentHandle as _;

pub(in crate::app) fn video_columns(view: VideoLayout) -> usize {
    match view {
        VideoLayout::List => 1,
        VideoLayout::TwoColumns => 2,
        VideoLayout::ThreeColumns => 3,
    }
}

pub(in crate::app) fn scroll_to_visible(
    scroll: f64,
    item_top: f64,
    item_bottom: f64,
    viewport_height: f64,
    content_height: f64,
) -> f64 {
    scroll
        .min(item_top)
        .max(item_bottom - viewport_height)
        .clamp(0.0, (content_height - viewport_height).max(0.0))
}

/// 选中行超出可见范围时调整首行
pub(in crate::app) fn ensure_visible(first: usize, selected: usize, visible: usize) -> usize {
    first
        .min(selected)
        .max(selected.saturating_sub(visible.saturating_sub(1)))
}

/// 视口能容纳的整行数
pub(in crate::app) fn visible_rows(video_height: f32, top: f32, row_height: f32) -> usize {
    (((video_height - top) / row_height) as usize).max(1)
}

pub(in crate::app) fn video_row(item: &model::Item) -> VideoRow {
    VideoRow {
        title: item.title.as_str().into(),
        author: item.meta.as_str().into(),
        duration: item.duration.as_str().into(),
        views: item.views.as_deref().unwrap_or_default().into(),
        likes: item.likes.as_deref().unwrap_or_default().into(),
        danmaku: item.danmaku.as_deref().unwrap_or_default().into(),
        progress: Default::default(),
        watched: Default::default(),
        date: item.date.as_deref().unwrap_or_default().into(),
    }
}

pub(in crate::app) fn compact_card_count(value: &str) -> slint::SharedString {
    if let Some((whole, fraction)) = value.split_once('.') {
        let unit = fraction.chars().last();
        if whole.len() >= 3 && matches!(unit, Some('万' | '亿')) {
            return format!("{}{}", whole, unit.unwrap()).into();
        }
    }
    value.into()
}

pub(in crate::app) fn sync_list(
    ui: &AppWindow,
    rows: Vec<VideoRow>,
    headers: Vec<crate::HistoryHeader>,
    tops: Vec<f32>,
) {
    let list_model = ui.global::<crate::ListModel>();
    list_model.set_rows(ModelRc::new(VecModel::from(rows)));
    list_model.set_headers(ModelRc::new(VecModel::from(headers)));
    list_model.set_row_tops(ModelRc::new(VecModel::from(tops)));
}

pub(in crate::app) fn list_source(tab: HomeTab, mine_detail: i32) -> Option<usize> {
    match tab {
        HomeTab::Recommended | HomeTab::Popular | HomeTab::Dynamic | HomeTab::Live => {
            Some(tab.index())
        }
        HomeTab::Mine if mine_detail >= 0 => Some(mine_detail as usize),
        _ => None,
    }
}

// 首屏刷新保留后续页、已解码封面和选中项的列表位置
pub(in crate::app) fn refresh_first_page(
    items: &mut Vec<model::Item>,
    covers: &mut Vec<Option<slint::Image>>,
    first_page_size: &mut usize,
    mut fresh: Vec<model::Item>,
    selection: i32,
) -> i32 {
    let images: HashMap<_, _> = items
        .iter()
        .zip(covers.iter())
        .map(|(item, image)| (item.bvid.clone(), image.clone()))
        .collect();
    let mut known: HashSet<_> = fresh.iter().map(|item| item.bvid.clone()).collect();
    let tail = items
        .drain((*first_page_size).min(items.len())..)
        .filter(|item| known.insert(item.bvid.clone()))
        .collect::<Vec<_>>();
    *first_page_size = fresh.len();
    fresh.extend(tail);
    *items = fresh;
    *covers = items
        .iter()
        .map(|item| images.get(&item.bvid).cloned().flatten())
        .collect();
    selection.clamp(0, (items.len() as i32 - 1).max(0))
}

impl App {
    pub(in crate::app) fn video_list_key(&mut self, key: &str, len: usize) -> bool {
        if key == "v" {
            self.view = self.view.cycle(1);
            self.settings_changed = true;
            return true;
        }
        let layout = self.view;
        let columns = video_columns(layout) as i32;
        let step = match key {
            "UP" => Some(-columns),
            "DOWN" => Some(columns),
            "LEFT" => Some(-1),
            "RIGHT" => Some(1),
            _ => None,
        };
        if let Some(step) = step {
            let next = if self.page == Page::Home
                && self.tab == HomeTab::Live
                && matches!(key, "UP" | "DOWN")
            {
                self.live_list.move_vertical(self.sel, step)
            } else {
                (self.sel + step).max(0)
            };
            if next < len as i32 {
                self.sel = next;
            }
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests;
