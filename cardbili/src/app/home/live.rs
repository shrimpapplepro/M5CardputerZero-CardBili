use super::super::*;
use slint::ComponentHandle as _;

const POPULAR_LIMIT: usize = 20;

#[derive(Default)]
pub(in crate::app) struct LiveState {
    pub areas: Vec<model::LiveArea>,
    pub filter: model::LiveArea,
    pub request: u64,
    pub following_count: usize,
    parent: Option<u64>,
}

impl LiveState {
    pub(in crate::app) fn layout(
        &self,
        len: usize,
        columns: usize,
        cell_height: f32,
        header_height: f32,
    ) -> LiveLayout {
        let following = self.following_count.min(len);
        let popular = (following + POPULAR_LIMIT).min(len);
        let mut headers = Vec::new();
        let mut tops = Vec::with_capacity(len);
        let mut top = 0.0;
        for (start, end, title) in [
            (0, following, "我的关注"),
            (following, popular, "热门直播"),
            (popular, len, "全部直播"),
        ] {
            if start == end {
                continue;
            }
            headers.push(crate::HistoryHeader {
                title: title.into(),
                top,
            });
            top += header_height;
            tops.extend((0..end - start).map(|index| top + (index / columns) as f32 * cell_height));
            top += (end - start).div_ceil(columns) as f32 * cell_height;
        }
        LiveLayout {
            headers,
            tops,
            header_height,
        }
    }

    fn section_start(&self, index: usize) -> usize {
        if index < self.following_count {
            0
        } else if index < self.following_count + POPULAR_LIMIT {
            self.following_count
        } else {
            self.following_count + POPULAR_LIMIT
        }
    }

    pub(in crate::app) fn column(&self, index: usize, columns: usize) -> usize {
        (index - self.section_start(index)) % columns
    }

    pub(in crate::app) fn move_vertical(&self, selected: i32, step: i32) -> i32 {
        let columns = step.unsigned_abs() as usize;
        let index = selected as usize;
        let start = self.section_start(index);
        let column = self.column(index, columns);
        if step > 0 {
            let end = if index < self.following_count {
                self.following_count
            } else if index < self.following_count + POPULAR_LIMIT {
                self.following_count + POPULAR_LIMIT
            } else {
                return selected + step;
            };
            let next_row = index + columns - column;
            if next_row >= end {
                return (end + column) as i32;
            }
            return (next_row + column).min(end - 1) as i32;
        }
        if index >= start + columns {
            return (index - columns) as i32;
        }
        if start == 0 {
            return 0;
        }
        // 跨区向上时保持列位置，末行缺列时选中该区最后一项
        let previous_start = self.section_start(start - 1);
        let last_row = start - 1 - (start - 1 - previous_start) % columns;
        (last_row + column).min(start - 1) as i32
    }

    fn options(&self) -> Vec<&model::LiveArea> {
        self.areas
            .iter()
            .filter(|area| match self.parent {
                Some(parent) => area.parent_id == parent,
                None => area.id == 0,
            })
            .collect()
    }
}

pub(in crate::app) struct LiveLayout {
    pub headers: Vec<crate::HistoryHeader>,
    pub tops: Vec<f32>,
    header_height: f32,
}

impl LiveLayout {
    pub fn selection_top(&self, selected: usize) -> f64 {
        let top = self.tops.get(selected).copied().unwrap_or_default();
        self.headers
            .iter()
            .find(|header| header.top + self.header_height == top)
            .map_or(top, |header| header.top) as f64
    }
}

impl App {
    pub(in crate::app) fn open_live_areas(&mut self) {
        self.direction_hold = None;
        self.live_list.parent =
            (self.live_list.filter.id > 0).then_some(self.live_list.filter.parent_id);
        self.ui.global::<crate::LiveAreas>().set_open(true);
        if self.live_list.areas.is_empty() {
            self.load_live_areas();
        } else {
            self.sync_live_areas();
        }
    }

    fn load_live_areas(&self) {
        let panel = self.ui.global::<crate::LiveAreas>();
        if panel.get_loading() {
            return;
        }
        panel.set_loading(true);
        panel.set_error("".into());
        let tx = self.tx.clone();
        thread::spawn(move || {
            crate::cache::set_force(true);
            let _ = tx.send(Msg::LiveAreas(crate::api::live::areas()));
        });
    }

    pub(in crate::app) fn live_areas_loaded(
        &mut self,
        result: Result<Vec<model::LiveArea>, String>,
    ) {
        self.ui.global::<crate::LiveAreas>().set_loading(false);
        match result {
            Ok(areas) => {
                self.live_list.areas = areas;
                self.sync_live_areas();
            }
            Err(error) => {
                crate::diagnostics::write(format_args!("[直播分区] {error}"));
                self.ui
                    .global::<crate::LiveAreas>()
                    .set_error("分区加载失败，按刷新重试".into());
            }
        }
    }

    fn sync_live_areas(&self) {
        let options = self.live_list.options();
        let panel = self.ui.global::<crate::LiveAreas>();
        let title = self.live_list.parent.and_then(|parent| {
            self.live_list
                .areas
                .iter()
                .find(|area| area.parent_id == parent && area.id == 0)
        });
        panel.set_title(title.map_or_else(
            || "选择直播分区".into(),
            |area| format!("选择分区 · {}", area.name).into(),
        ));
        panel.set_selected(
            options
                .iter()
                .position(|area| {
                    area.parent_id == self.live_list.filter.parent_id
                        && area.id == self.live_list.filter.id
                })
                .unwrap_or(0) as i32,
        );
        panel.set_rows(ModelRc::new(VecModel::from(
            options
                .iter()
                .map(|area| {
                    if self.live_list.parent.is_some() && area.id == 0 {
                        "全部".into()
                    } else {
                        area.name.as_str().into()
                    }
                })
                .collect::<Vec<slint::SharedString>>(),
        )));
    }

    pub(in crate::app) fn live_area_key(&mut self, key: &str) {
        let panel = self.ui.global::<crate::LiveAreas>();
        if matches!(key, "ESC" | "BACKSPACE") {
            if self.live_list.parent.take().is_some() {
                self.sync_live_areas();
            } else {
                panel.set_open(false);
            }
            self.direction_hold = None;
            self.return_hold = None;
            return;
        }
        if key == "r" {
            self.load_live_areas();
            return;
        }
        if panel.get_loading() || !panel.get_error().is_empty() {
            return;
        }
        let options = self.live_list.options();
        let last = (options.len() as i32 - 1).max(0);
        let selected = panel.get_selected();
        let step = match key {
            "UP" => -3,
            "DOWN" => 3,
            "LEFT" => -1,
            "RIGHT" => 1,
            _ => 0,
        };
        if step != 0 {
            panel.set_selected((selected + step).clamp(0, last));
            return;
        }
        let index = if matches!(key, "ENTER" | "SPACE") {
            Some(selected as usize)
        } else {
            key.strip_prefix("AREA_SELECT:")
                .and_then(|index| index.parse().ok())
        };
        let Some(area) = index
            .and_then(|index| options.get(index))
            .map(|area| (*area).clone())
        else {
            return;
        };
        if self.live_list.parent.is_none() && area.parent_id > 0 {
            self.live_list.parent = Some(area.parent_id);
            self.sync_live_areas();
            return;
        }
        panel.set_open(false);
        self.direction_hold = None;
        if area.parent_id == self.live_list.filter.parent_id && area.id == self.live_list.filter.id
        {
            return;
        }
        self.live_list.filter = area;
        self.live_list.following_count = 0;
        let source = HomeTab::Live.index();
        // 切换分区时清空旧列表，刷新与翻页共用新的请求编号
        self.lists[source].clear();
        self.covers[source].clear();
        self.loaded[source] = false;
        self.pages[source] = 1;
        self.first_page_sizes[source] = 0;
        self.has_more[source] = true;
        self.sel = 0;
        self.list_scroll = 0.0;
        self.grid_scroll = 0.0;
        self.home_dirty = true;
        self.loading = true;
        self.status = "加载中…".into();
        self.status_detail.clear();
        self.load_home_list(source, false);
    }

    pub(in crate::app) fn live_list_loaded(
        &mut self,
        request: u64,
        page: u32,
        result: Result<model::LivePage, String>,
    ) {
        let source = HomeTab::Live.index();
        if request != self.live_list.request
            || (page > 1 && page as usize != self.pages[source] + 1)
        {
            return;
        }
        match result {
            Ok(data) => {
                let popular = data.popular;
                if page == 1 {
                    let update_more = self.pages[source] <= 1 && !self.loading_more[source];
                    let mut items = data.following;
                    self.live_list.following_count = items.len();
                    let mut known: HashSet<_> = items.iter().map(|item| item.bvid.clone()).collect();
                    items.extend(
                        popular
                            .items
                            .into_iter()
                            .filter(|item| known.insert(item.bvid.clone())),
                    );
                    self.list_loaded(source, items);
                    if update_more {
                        self.has_more[source] = popular.has_more;
                    }
                } else {
                    self.list_more(source, popular.items);
                    self.pages[source] = page as usize;
                    self.has_more[source] = popular.has_more;
                }
            }
            Err(error) => self.list_error(source, error),
        }
    }
}

#[cfg(test)]
mod tests;
