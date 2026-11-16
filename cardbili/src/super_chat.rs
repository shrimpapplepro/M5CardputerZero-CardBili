use serde_json::Value;
use std::collections::{HashSet, VecDeque};

#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub id: u64,
    pub price: u64,
    pub start: u64,
    pub end: u64,
    pub user: String,
    pub text: String,
    pub accent: u32,
    pub background: u32,
    pub foreground: u32,
}

impl Message {
    pub fn parse(value: &Value) -> Option<Self> {
        let price = number(&value["price"])?;
        let (accent, background) = match price {
            2000.. => (0xAB1A32, 0xFFD8D8),
            1000.. => (0xE54D4D, 0xFFE7E4),
            500.. => (0xE09443, 0xFFEAD2),
            100.. => (0xE2B52B, 0xFFF1C5),
            50.. => (0x427D9E, 0xDBFFFD),
            _ => (0x2A60B2, 0xEDF5FF),
        };
        Some(Self {
            id: number(&value["id"])?,
            price,
            start: number(&value["start_time"])?,
            end: number(&value["end_time"])?,
            user: value["user_info"]["uname"]
                .as_str()
                .unwrap_or_default()
                .into(),
            text: value["message"].as_str()?.into(),
            accent: color(&value["background_bottom_color"]).unwrap_or(accent),
            background: color(&value["background_color"]).unwrap_or(background),
            foreground: color(&value["message_font_color"]).unwrap_or(0xFFFFFF),
        })
    }

    pub fn visible(&self, now: u64) -> bool {
        self.start <= now && now < self.end
    }

    pub fn announcement(&self) -> crate::danmaku::Dm {
        crate::danmaku::Dm {
            t: 0.0,
            mode: 1,
            color: self.accent,
            text: format!("【¥{}】{}的醒目留言：{}", self.price, self.user, self.text),
        }
    }
}

fn number(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| value.as_str()?.parse().ok())
}

fn color(value: &Value) -> Option<u32> {
    let hex = value.as_str()?.strip_prefix('#')?;
    (hex.len() == 6)
        .then(|| u32::from_str_radix(hex, 16).ok())
        .flatten()
}

pub enum Update {
    Initial(Result<Vec<Message>, String>),
    Add(Message),
    Delete(Vec<u64>),
}

pub fn parse_update(value: &Value) -> Option<Update> {
    match value["cmd"].as_str()?.split(':').next()? {
        "SUPER_CHAT_MESSAGE" => Message::parse(&value["data"]).map(Update::Add),
        "SUPER_CHAT_MESSAGE_DELETE" => Some(Update::Delete(
            value["data"]["ids"]
                .as_array()?
                .iter()
                .filter_map(number)
                .collect(),
        )),
        _ => None,
    }
}

#[derive(Default)]
pub struct State {
    pub messages: Vec<Message>,
    pub loaded: bool,
    pub failed: bool,
    pub displayed_at: u64,
    pub announcements: VecDeque<(u64, crate::danmaku::Dm)>,
    touched: HashSet<u64>,
    checked_at: u64,
}

impl State {
    pub fn apply(&mut self, update: Update) -> bool {
        let fresh = match &update {
            Update::Add(message) => !self.messages.iter().any(|item| item.id == message.id),
            _ => false,
        };
        match update {
            Update::Initial(result) => {
                self.loaded = true;
                self.failed = result.is_err();
                if let Ok(messages) = result {
                    self.messages.extend(
                        messages
                            .into_iter()
                            .filter(|message| !self.touched.contains(&message.id)),
                    );
                }
                self.touched.clear();
            }
            Update::Add(message) => {
                if !self.loaded {
                    self.touched.insert(message.id);
                }
                self.messages.retain(|item| item.id != message.id);
                if fresh {
                    self.announcements
                        .push_back((message.id, message.announcement()));
                }
                self.messages.push(message);
            }
            Update::Delete(ids) => {
                if !self.loaded {
                    self.touched.extend(ids.iter().copied());
                }
                self.messages.retain(|item| !ids.contains(&item.id));
            }
        }
        self.messages
            .sort_by_key(|item| std::cmp::Reverse((item.start, item.id)));
        let mut seen = HashSet::new();
        self.messages.retain(|item| seen.insert(item.id));
        fresh
    }

    pub fn expire(&mut self, now: u64, retain_expired: bool) -> bool {
        let count = self.messages.len();
        let activated = self
            .messages
            .iter()
            .any(|item| self.checked_at < item.start && item.start <= now && now < item.end);
        // 弹窗打开期间保留到期留言，关闭时统一清理
        if !retain_expired {
            self.messages.retain(|item| item.end > now);
        }
        self.announcements.retain(|(id, _)| {
            self.messages
                .iter()
                .any(|item| item.id == *id && item.visible(now))
        });
        self.checked_at = now;
        activated || count != self.messages.len()
    }
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests;
