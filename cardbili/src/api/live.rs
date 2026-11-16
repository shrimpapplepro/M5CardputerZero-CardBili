use super::{api_err, cache, model, net, Item, Value};

pub fn list(
    page: u32,
    area: &model::LiveArea,
    cookies: Option<&str>,
) -> Result<model::LivePage, String> {
    let popular = rooms(page, area)?;
    let following = if let (1, Some(cookies)) = (page, cookies) {
        following(area, cookies).unwrap_or_else(|error| {
            crate::diagnostics::write(format_args!("[关注直播] {error}"));
            Vec::new()
        })
    } else {
        Vec::new()
    };
    Ok(model::LivePage { following, popular })
}

fn following(area: &model::LiveArea, cookies: &str) -> Result<Vec<Item>, String> {
    let mut items = Vec::new();
    let mut known = std::collections::HashSet::new();
    let mut page = 1;
    let mut received = 0;
    loop {
        let url = format!(
            "https://api.live.bilibili.com/xlive/web-ucenter/v1/xfetter/GetWebList?page={page}&page_size=20"
        );
        let value = cache::api_json(&url, Some(cookies), cache::TTL_LIST)?;
        if value["code"].as_i64() != Some(0) {
            return Err(api_err("关注直播", &value));
        }
        let data = &value["data"];
        let Some(rooms) = data["rooms"].as_array().filter(|rooms| !rooms.is_empty()) else {
            break;
        };
        received += rooms.len() as u64;
        for room in rooms {
            let room_id = room["room_id"].as_u64().unwrap_or(0);
            if room_id == 0 || !known.insert(room_id) || !following_in_area(room, area) {
                continue;
            }
            let pic = ["cover_from_user", "keyframe"]
                .iter()
                .find_map(|key| room[*key].as_str().filter(|value| !value.is_empty()))
                .unwrap_or_default();
            items.push(Item {
                title: room["title"].as_str().unwrap_or_default().into(),
                meta: room["uname"].as_str().unwrap_or_default().into(),
                bvid: room_id.to_string(),
                pic: pic.into(),
                views: room["online"].as_i64().map(model::fmt_num),
                ..Default::default()
            });
        }
        if received >= data["count"].as_u64().unwrap_or(received) {
            break;
        }
        page += 1;
    }
    Ok(items)
}

fn following_in_area(room: &Value, area: &model::LiveArea) -> bool {
    room["live_status"].as_i64() == Some(1)
        && (area.parent_id == 0 || room["area_v2_parent_id"].as_u64() == Some(area.parent_id))
        && (area.id == 0 || room["area_v2_id"].as_u64() == Some(area.id))
}

// 分区目录同时保留一级分区与子分区
pub fn areas() -> Result<Vec<model::LiveArea>, String> {
    let value = cache::api_json(
        "https://api.live.bilibili.com/room/v1/Area/getList",
        None,
        cache::TTL_LIST,
    )?;
    if value["code"].as_i64() != Some(0) {
        return Err(api_err("直播分区", &value));
    }
    let mut areas = vec![model::LiveArea {
        name: "全部".into(),
        ..Default::default()
    }];
    for parent in value["data"].as_array().into_iter().flatten() {
        let Some(parent_id) = parent["id"].as_u64() else {
            continue;
        };
        areas.push(model::LiveArea {
            parent_id,
            id: 0,
            name: parent["name"].as_str().unwrap_or_default().into(),
        });
        for child in parent["list"].as_array().into_iter().flatten() {
            let Some(id) = child["id"].as_str().and_then(|id| id.parse().ok()) else {
                continue;
            };
            areas.push(model::LiveArea {
                parent_id,
                id,
                name: child["name"].as_str().unwrap_or_default().into(),
            });
        }
    }
    Ok(areas)
}

// 全部直播使用推荐列表，指定分区按人气排序
fn rooms(page: u32, area: &model::LiveArea) -> Result<model::VideoPage, String> {
    let filtered = area.parent_id > 0;
    let url = if filtered {
        format!(
            "https://api.live.bilibili.com/room/v3/area/getRoomList?platform=web&parent_area_id={}&area_id={}&sort_type=online&page={page}&page_size=20",
            area.parent_id, area.id
        )
    } else {
        format!("https://api.live.bilibili.com/room/v1/room/get_user_recommend?page={page}&page_size=20")
    };
    let value = cache::api_json(&url, None, cache::TTL_LIST)?;
    if value["code"].as_i64() != Some(0) {
        return Err(api_err("直播列表", &value));
    }
    let data = &value["data"];
    let list = if filtered { &data["list"] } else { data };
    let mut items = Vec::new();
    for room in list.as_array().into_iter().flatten() {
        let room_id = room["roomid"].as_u64().unwrap_or(0);
        if room_id == 0 {
            continue;
        }
        let pic = ["user_cover", "system_cover", "keyframe"]
            .iter()
            .find_map(|key| room[*key].as_str().filter(|value| !value.is_empty()))
            .unwrap_or_default();
        items.push(Item {
            title: room["title"].as_str().unwrap_or_default().into(),
            meta: room["uname"].as_str().unwrap_or_default().into(),
            bvid: room_id.to_string(),
            pic: pic.into(),
            views: room["online"].as_i64().map(model::fmt_num),
            ..Default::default()
        });
    }
    let total = data["count"].as_u64();
    let has_more = !items.is_empty() && total.map_or(true, |total| u64::from(page) * 20 < total);
    Ok(model::VideoPage {
        items,
        has_more,
        total,
    })
}

// 下播与不支持的编码属于终止状态，网络请求失败可重新获取地址
#[derive(Debug)]
pub enum RoomError {
    Offline,
    Unsupported,
    Request(String),
}

impl From<String> for RoomError {
    fn from(error: String) -> Self {
        Self::Request(error)
    }
}

impl std::fmt::Display for RoomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Offline => f.write_str("直播间未开播"),
            Self::Unsupported => f.write_str("暂无兼容的 AVC 直播流，请换个直播间"),
            Self::Request(error) => f.write_str(error),
        }
    }
}

pub fn audience(room_id: u64, cookies: Option<&str>) -> Result<String, String> {
    let url = format!(
        "https://api.live.bilibili.com/xlive/web-room/v1/index/getInfoByRoom?room_id={room_id}"
    );
    let value = net::get_json(&url, cookies)?;
    if value["code"].as_i64() != Some(0) {
        return Err(api_err("在线观众", &value));
    }
    let rank = &value["data"]["room_rank_info"]["user_rank_entry"]["user_contribution_rank_entry"];
    rank["count_text"]
        .as_str()
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
        .or_else(|| rank["count"].as_i64().map(model::fmt_num))
        .ok_or_else(|| "直播间未提供在线观众数".into())
}

pub fn room(room_id: u64, cookies: Option<&str>) -> Result<model::LiveInfo, RoomError> {
    let info_url = format!(
        "https://api.live.bilibili.com/xlive/web-room/v1/index/getH5InfoByRoom?room_id={}",
        room_id
    );
    let v = net::get_json(&info_url, cookies)?;
    if v["code"].as_i64() != Some(0) {
        return Err(api_err("直播间信息", &v).into());
    }
    let room = &v["data"]["room_info"];
    if room["live_status"].as_i64() != Some(1) {
        return Err(RoomError::Offline);
    }
    let room_id = room["room_id"]
        .as_u64()
        .filter(|id| *id > 0)
        .unwrap_or(room_id);
    let play_url = format!(
        "https://api.live.bilibili.com/xlive/web-room/v2/index/getRoomPlayInfo?room_id={}&protocol=0,1&format=0,1,2&codec=0&qn=150&platform=web&ptype=8&dolby=5&panorama=1",
        room_id
    );
    let p = net::get_json(&play_url, cookies)?;
    if p["code"].as_i64() != Some(0) {
        return Err(api_err("直播播放地址", &p).into());
    }
    let url = stream_url(&p["data"]).ok_or(RoomError::Unsupported)?;
    crate::diagnostics::write(format_args!(
        "[直播] 房间={} 流={}",
        room_id,
        crate::cdn::host(&url)
    ));
    Ok(model::LiveInfo {
        room_id,
        title: room["title"].as_str().unwrap_or_default().into(),
        uname: v["data"]["anchor_info"]["base_info"]["uname"]
            .as_str()
            .unwrap_or_default()
            .into(),
        online: model::fmt_num(room["online"].as_i64().unwrap_or(0)),
        play_url: url,
        started_at: room["live_start_time"].as_i64().unwrap_or(0),
    })
}

// 先选较低画质，再选连续传输的 FLV，设备解码器支持 AVC
fn stream_url(data: &Value) -> Option<String> {
    let mut best = None;
    for stream in data["playurl_info"]["playurl"]["stream"]
        .as_array()
        .into_iter()
        .flatten()
    {
        for format in stream["format"].as_array().into_iter().flatten() {
            let priority = match (
                stream["protocol_name"].as_str(),
                format["format_name"].as_str(),
            ) {
                (Some("http_stream"), Some("flv")) => 0,
                (Some("http_hls"), Some("ts")) => 1,
                (Some("http_hls"), Some("fmp4")) => 2,
                _ => continue,
            };
            for codec in format["codec"].as_array().into_iter().flatten() {
                if codec["codec_name"].as_str() != Some("avc") {
                    continue;
                }
                let base = codec["base_url"].as_str().unwrap_or_default();
                let quality = codec["current_qn"].as_u64().unwrap_or(u64::MAX);
                for server in codec["url_info"].as_array().into_iter().flatten() {
                    let host = server["host"].as_str().unwrap_or_default();
                    let extra = server["extra"].as_str().unwrap_or_default();
                    if host.is_empty() || base.is_empty() {
                        continue;
                    }
                    let rank = (quality, priority);
                    if best.as_ref().map_or(true, |(previous, _)| rank < *previous) {
                        best = Some((rank, format!("{host}{base}{extra}")));
                    }
                }
            }
        }
    }
    best.map(|(_, url)| url)
}

pub struct DanmakuServer {
    pub host: String,
    pub port: u16,
}

pub fn super_chat_messages(
    room: u64,
    cookies: Option<&str>,
) -> Result<Vec<crate::super_chat::Message>, String> {
    let url = format!(
        "https://api.live.bilibili.com/av/v1/SuperChat/getMessageList?room_id={room}"
    );
    let value = net::get_json(&url, cookies)?;
    if value["code"].as_i64() != Some(0) {
        return Err(api_err("醒目留言", &value));
    }
    Ok(value["data"]["list"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(crate::super_chat::Message::parse)
        .collect())
}

pub struct DanmakuInfo {
    pub token: String,
    pub servers: Vec<DanmakuServer>,
}

pub fn danmaku_info(room_id: u64, cookies: Option<&str>) -> Result<DanmakuInfo, String> {
    let params = super::wbi::wbi_sign(
        &[
            ("id".into(), room_id.to_string()),
            ("web_location".into(), "444.8".into()),
        ],
        &super::wbi::fetch_mixin()?,
    );
    let url = format!(
        "https://api.live.bilibili.com/xlive/web-room/v1/index/getDanmuInfo?{}",
        net::query_string(&params)
    );
    let value = net::get_json(&url, cookies)?;
    if value["code"].as_i64() != Some(0) {
        return Err(api_err("直播弹幕", &value));
    }
    let data = &value["data"];
    let token = data["token"]
        .as_str()
        .filter(|token| !token.is_empty())
        .ok_or("直播弹幕认证信息为空")?
        .to_string();
    let servers = data["host_list"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|server| {
            Some(DanmakuServer {
                host: server["host"].as_str()?.to_string(),
                port: u16::try_from(server["wss_port"].as_u64()?).ok()?,
            })
        })
        .collect::<Vec<_>>();
    if servers.is_empty() {
        return Err("暂无可用的直播弹幕服务器".into());
    }
    Ok(DanmakuInfo { token, servers })
}

#[cfg(test)]
mod tests;
