pub mod auth;
pub mod favorites;
pub mod live;
pub use live::room as live_room;
pub mod playback;
pub mod sections;
mod wbi;

use crate::cache;
use crate::model::{self, Account, Detail, Item, Owner, PlayInfo};
use crate::net;
use base64::Engine;
use serde_json::Value;
use std::io::Read;

/// 统一生成友好的接口错误文案，保留业务码与原始错误信息
fn api_err(name: &str, v: &Value) -> String {
    let code = v["code"].as_i64().unwrap_or(-1);
    let msg = v["message"].as_str().unwrap_or("");
    crate::diagnostics::write(format_args!("[接口] {name} 失败 code={code} message={msg}"));
    if msg.is_empty() {
        format!("{}加载失败\ncode={}", name, code)
    } else {
        format!("{}加载失败\ncode={} {}", name, code, msg)
    }
}

fn count_text(value: &Value) -> Option<String> {
    value.as_i64().map(model::fmt_num).or_else(|| {
        value
            .as_str()
            .filter(|text| !text.is_empty())
            .map(str::to_string)
    })
}

fn owner(value: &Value) -> Owner {
    Owner {
        mid: value["mid"]
            .as_i64()
            .map(|mid| mid.to_string())
            .unwrap_or_default(),
        name: value["name"].as_str().unwrap_or_default().to_string(),
    }
}

pub fn popular(cookies: Option<&str>, pn: u32) -> Result<Vec<Item>, String> {
    let url = format!(
        "https://api.bilibili.com/x/web-interface/popular?ps=20&pn={}",
        pn
    );
    let v = cache::api_json(&url, cookies, cache::TTL_LIST)?;
    if v["code"].as_i64() != Some(0) {
        return Err(api_err("热门推荐", &v));
    }
    Ok(video_items(&v["data"]["list"]))
}

fn video_items(value: &Value) -> Vec<Item> {
    let mut out = Vec::new();
    if let Some(list) = value.as_array() {
        for it in list {
            let bvid = it["bvid"].as_str().unwrap_or("").to_string();
            if bvid.is_empty() {
                continue;
            }
            let title = it["title"].as_str().unwrap_or("").to_string();
            let name = it["owner"]["name"].as_str().unwrap_or("");
            let d = model::dur_of(&it["duration"]);
            let pic = it["pic"].as_str().unwrap_or("").to_string();
            out.push(Item {
                title,
                meta: name.to_string(),
                duration: model::mmss(d),
                bvid,
                pic,
                views: count_text(&it["stat"]["view"]),
                likes: count_text(&it["stat"]["like"]),
                danmaku: count_text(&it["stat"]["danmaku"]),
                ..Default::default()
            });
        }
    }
    out
}

pub fn rcmd(cookies: Option<&str>, page: u32) -> Result<Vec<Item>, String> {
    let base = vec![
        ("ps".to_string(), "20".to_string()),
        ("fresh_type".to_string(), "4".to_string()),
        ("feed_version".to_string(), "V8".to_string()),
        ("fresh_idx".to_string(), page.to_string()),
        ("fresh_idx_1h".to_string(), page.to_string()),
        ("fetch_row".to_string(), page.to_string()),
        ("brush".to_string(), "0".to_string()),
        ("homepage_ver".to_string(), "1".to_string()),
        ("y_num".to_string(), "5".to_string()),
        ("last_y_num".to_string(), "5".to_string()),
        ("web_location".to_string(), "1430650".to_string()),
    ];
    let endpoint = "https://api.bilibili.com/x/web-interface/wbi/index/top/feed/rcmd";
    let cache_key = format!("{}?{}", endpoint, net::query_string(&base));
    let v = cache::api_json_with(&cache_key, cookies, cache::TTL_LIST, || {
        let mixin = wbi::fetch_mixin()?;
        let signed = wbi::wbi_sign(&base, &mixin);
        let url = format!("{}?{}", endpoint, net::query_string(&signed));
        net::get(&url, cookies)
    })?;
    if v["code"].as_i64() != Some(0) {
        return Err(api_err("推荐", &v));
    }
    Ok(video_items(&v["data"]["item"]))
}

pub const HISTORY_URL: &str =
    "https://api.bilibili.com/x/web-interface/history/cursor?max=0&view_at=0&business=";

pub fn history(
    cookies: &str,
    cursor: Option<&model::HistoryCursor>,
) -> Result<model::HistoryPage, String> {
    let url = cursor
        .map(|cursor| {
            format!(
                "https://api.bilibili.com/x/web-interface/history/cursor?{}",
                net::query_string(&[
                    ("max".into(), cursor.max.to_string()),
                    ("view_at".into(), cursor.view_at.to_string()),
                    ("business".into(), cursor.business.clone())
                ])
            )
        })
        .unwrap_or_else(|| HISTORY_URL.into());
    let v = cache::api_json(&url, Some(cookies), cache::TTL_LIST)?;
    if v["code"].as_i64() != Some(0) {
        return Err(api_err("历史记录", &v));
    }
    let mut out = Vec::new();
    if let Some(list) = v["data"]["list"].as_array() {
        for it in list {
            let bvid = it["history"]["bvid"].as_str().unwrap_or("").to_string();
            if bvid.is_empty() {
                continue;
            }
            let lt = it["long_title"].as_str().unwrap_or("");
            let title = if !lt.is_empty() {
                lt.to_string()
            } else {
                it["title"].as_str().unwrap_or("").to_string()
            };
            let name = it["author_name"].as_str().unwrap_or("");
            let d = model::dur_of(&it["duration"]);
            let pic = it["cover"].as_str().unwrap_or("").to_string();
            out.push(Item {
                title,
                meta: name.to_string(),
                duration: model::mmss(d),
                bvid,
                pic,
                views: count_text(&it["stat"]["view"]),
                likes: count_text(&it["stat"]["like"]),
                danmaku: count_text(&it["stat"]["danmaku"]),
                progress: it["progress"].as_i64().unwrap_or_default(),
                view_at: it["view_at"].as_i64().unwrap_or_default(),
                date: None,
                duration_secs: d,
            });
        }
    }
    let next = &v["data"]["cursor"];
    let next = model::HistoryCursor {
        max: next["max"].as_i64().unwrap_or_default(),
        view_at: next["view_at"].as_i64().unwrap_or_default(),
        business: next["business"].as_str().unwrap_or_default().into(),
    };
    let has_more = v["data"]["list"]
        .as_array()
        .is_some_and(|items| !items.is_empty())
        && (next.max > 0 || next.view_at > 0)
        && cursor
            .is_none_or(|previous| previous.max != next.max || previous.view_at != next.view_at);
    Ok(model::HistoryPage {
        items: out,
        cursor: next,
        has_more,
    })
}

pub fn dynamic(cookies: &str, offset: &str) -> Result<model::DynamicPage, String> {
    let params = net::query_string(&[("offset".into(), offset.into())]);
    let url = format!("https://api.bilibili.com/x/polymer/web-dynamic/v1/feed/all?type=video&{params}&features=itemOpusStyle,listOnlyfans,opusBigCover,onlyfansVote");
    let v = cache::api_json(&url, Some(cookies), cache::TTL_LIST)?;
    if v["code"].as_i64() != Some(0) {
        return Err(api_err("动态", &v));
    }
    let mut out = Vec::new();
    if let Some(items) = v["data"]["items"].as_array() {
        for it in items {
            if it["type"].as_str() != Some("DYNAMIC_TYPE_AV") {
                continue;
            }
            let archive = &it["modules"]["module_dynamic"]["major"]["archive"];
            if !archive.is_object() {
                continue;
            }
            let bvid = archive["bvid"].as_str().unwrap_or("");
            if bvid.is_empty() {
                continue;
            }
            let title = archive["title"].as_str().unwrap_or("").to_string();
            let author = it["modules"]["module_author"]["name"]
                .as_str()
                .unwrap_or("");
            let dtext = archive["duration_text"].as_str().unwrap_or("");
            let pic = archive["cover"].as_str().unwrap_or("").to_string();
            out.push(Item {
                title,
                meta: author.to_string(),
                duration: dtext.to_string(),
                views: count_text(&archive["stat"]["play"]),
                likes: count_text(&archive["stat"]["like"]),
                danmaku: count_text(&archive["stat"]["danmaku"]),
                bvid: bvid.to_string(),
                pic,
                ..Default::default()
            });
        }
    }
    let next = v["data"]["offset"].as_str().unwrap_or_default().to_string();
    let more = &v["data"]["has_more"];
    let has_more = (more.as_bool() == Some(true) || more.as_i64() == Some(1))
        && !next.is_empty()
        && next != offset;
    Ok(model::DynamicPage {
        items: out,
        offset: next,
        has_more,
    })
}

pub fn play_info(
    bvid: &str,
    selected_cid: Option<i64>,
    cookies: Option<&str>,
    cdn: &str,
) -> Result<PlayInfo, String> {
    let vurl = format!(
        "https://api.bilibili.com/x/web-interface/view?bvid={}",
        bvid
    );
    let v = cache::api_json(&vurl, cookies, cache::TTL_DETAIL)?;
    if v["code"].as_i64() != Some(0) {
        return Err(api_err("视频信息", &v));
    }
    let d = &v["data"];
    let parts = sections::parts(d);
    let cid = selected_cid
        .or_else(|| d["cid"].as_i64())
        .ok_or("视频缺少分 P 信息")?;
    let part = parts.iter().find(|part| part.cid == cid);
    let title = if parts.len() > 1 {
        part.map(|part| part.title.clone()).unwrap_or_default()
    } else {
        d["title"].as_str().unwrap_or_default().into()
    };
    let p = play_urls(bvid, cid, cookies)?;
    let dash = &p["data"]["dash"];
    let video = dash["video"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|stream| stream["codecid"].as_i64() == Some(7))
        .min_by_key(|stream| stream["bandwidth"].as_u64().unwrap_or(u64::MAX))
        .ok_or("无可用 H.264 视频流")?;
    let audio = dash["audio"]
        .as_array()
        .into_iter()
        .flatten()
        .min_by_key(|stream| stream["bandwidth"].as_u64().unwrap_or(u64::MAX))
        .ok_or("无可用音频流")?;
    let cdn = &crate::cdn::OPTIONS[crate::cdn::index(cdn)];
    let video = cdn.url(video)?;
    let audio = cdn.url(audio)?;
    crate::diagnostics::write(format_args!(
        "[CDN] 播放选路 线路={} 视频={} 音频={}",
        cdn.id,
        crate::cdn::host(&video),
        crate::cdn::host(&audio)
    ));
    let duration = model::dur_of(&p["data"]["timelength"]) / 1000.0;
    Ok(PlayInfo {
        aid: d["aid"].as_i64().unwrap_or(0),
        bvid: bvid.to_string(),
        owner: owner(&d["owner"]),
        title,
        video_url: video.into(),
        audio_url: audio.into(),
        duration,
        cid,
        parts,
        live: false,
    })
}

pub fn play_urls(bvid: &str, cid: i64, cookies: Option<&str>) -> Result<Value, String> {
    let purl = format!(
        "https://api.bilibili.com/x/player/playurl?bvid={}&cid={}&qn=16&fnval=16&fourk=0",
        bvid, cid
    );
    let p = cache::api_json(&purl, cookies, cache::TTL_PLAYURL)?;
    if p["code"].as_i64() != Some(0) {
        return Err(api_err("播放地址", &p));
    }
    Ok(p)
}

// 拉取视频弹幕 XML 并解析为条目
pub fn danmaku_list(cid: i64, cookies: Option<&str>) -> Result<Vec<crate::danmaku::Dm>, String> {
    let url = format!("https://api.bilibili.com/x/v1/dm/list.so?oid={}", cid);
    let xml = net::get(&url, cookies)?;
    Ok(parse_danmaku(&xml))
}

fn parse_danmaku(xml: &str) -> Vec<crate::danmaku::Dm> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<d p=\"") {
        let s = &rest[i + 6..];
        let q = match s.find('"') {
            Some(q) => q,
            None => break,
        };
        let params = &s[..q];
        let after = &s[q + 1..];
        let close = match after.find("</d>") {
            Some(c) => c,
            None => break,
        };
        let inner = &after[..close];
        let body = match inner.find('>') {
            Some(g) => &inner[g + 1..],
            None => inner,
        };
        let text = unescape_xml(body);
        if let Some(dm) = parse_params(params, text) {
            out.push(dm);
        }
        rest = &after[close + 4..];
    }
    out
}

fn parse_params(p: &str, text: String) -> Option<crate::danmaku::Dm> {
    let mut it = p.split(',');
    let t: f64 = it.next()?.parse().ok()?;
    let mode: u8 = it.next()?.parse().ok()?;
    let _size = it.next();
    let color: u32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(16777215);
    Some(crate::danmaku::Dm {
        t,
        mode,
        color,
        text,
    })
}

// 解码命名字符实体与十进制/十六进制数字实体
pub(crate) fn unescape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos + 1..];
        let decoded = tail
            .find(';')
            .filter(|&end| end <= 10)
            .and_then(|end| {
                let ch = match &tail[..end] {
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    "amp" => Some('&'),
                    "nbsp" => Some(' '),
                    entity => entity.strip_prefix('#').and_then(|num| match num {
                        _ if num.starts_with('x') || num.starts_with('X') => {
                            u32::from_str_radix(&num[1..], 16).ok().and_then(char::from_u32)
                        }
                        _ => num.parse::<u32>().ok().and_then(char::from_u32),
                    }),
                };
                ch.map(|c| (c, end + 1))
            });
        match decoded {
            Some((c, consumed)) => {
                out.push(c);
                rest = &tail[consumed..];
            }
            None => {
                out.push('&');
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

pub fn account(cookies: Option<&str>) -> Result<Account, String> {
    let v = cache::api_json(
        "https://api.bilibili.com/x/web-interface/nav",
        cookies,
        cache::TTL_ACCOUNT,
    )?;
    if v["code"].as_i64() != Some(0) {
        return Err(api_err("账号信息", &v));
    }
    let d = &v["data"];
    Ok(Account {
        logged: d["isLogin"].as_bool().unwrap_or(false),
        name: d["uname"].as_str().unwrap_or("").to_string(),
        mid: d["mid"].as_i64().map(|x| x.to_string()).unwrap_or_default(),
        level: d["level_info"]["current_level"].as_i64().unwrap_or(0),
        vip: if d["vipStatus"].as_i64().unwrap_or(0) > 0 {
            "大会员".to_string()
        } else {
            String::new()
        },
        face: d["face"].as_str().unwrap_or("").to_string(),
    })
}

pub fn video_detail(bvid: &str, cookies: Option<&str>) -> Result<Detail, String> {
    let vurl = format!(
        "https://api.bilibili.com/x/web-interface/view?bvid={}",
        bvid
    );
    let v = cache::api_json(&vurl, cookies, cache::TTL_DETAIL)?;
    if v["code"].as_i64() != Some(0) {
        return Err(api_err("视频信息", &v));
    }
    let d = &v["data"];
    let aid = d["aid"].as_i64().unwrap_or(0);
    let stat = &d["stat"];

    Ok(Detail {
        aid,
        cid: d["cid"].as_i64().unwrap_or_default(),
        parts: sections::parts(d),
        title: d["title"].as_str().unwrap_or("").to_string(),
        pic: d["pic"].as_str().unwrap_or("").to_string(),
        duration: model::dur_of(&d["duration"]),
        owner: owner(&d["owner"]),
        views: stat["view"].as_i64().unwrap_or(0),
        likes: stat["like"].as_i64().unwrap_or(0),
        coins: stat["coin"].as_i64().unwrap_or(0),
        favorites: stat["favorite"].as_i64().unwrap_or(0),
        danmaku: stat["danmaku"].as_i64().unwrap_or(0),
        desc: d["desc"].as_str().unwrap_or("").to_string(),
    })
}

fn random_base64(length: usize) -> Result<String, String> {
    let mut bytes = vec![0; length];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|error| format!("请求参数生成失败\n{error}"))?;
    let mut encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    // PiliPlus 的随机参数保留 Base64 编码除末尾两字符之外的内容
    encoded.truncate(encoded.len() - 2);
    Ok(encoded)
}

pub fn uploader_videos(
    mid: &str,
    page: u32,
    cookies: Option<&str>,
) -> Result<model::VideoPage, String> {
    let endpoint = "https://api.bilibili.com/x/space/wbi/arc/search";
    let params = vec![
        ("mid".into(), mid.to_string()),
        ("pn".into(), page.to_string()),
        ("ps".into(), "20".into()),
        ("order".into(), "pubdate".into()),
        ("tid".into(), "0".into()),
        ("platform".into(), "web".into()),
        ("web_location".into(), "333.1387".into()),
        ("order_avoided".into(), "true".into()),
        ("dm_img_list".into(), "[]".into()),
        (
            "dm_img_inter".into(),
            r#"{"ds":[],"wh":[0,0,0],"of":[0,0,0]}"#.into(),
        ),
    ];
    let key = format!("{}?{}", endpoint, net::query_string(&params));
    let value = cache::api_json_with(&key, cookies, cache::TTL_LIST, || {
        let mut params = params;
        params.push(("dm_img_str".into(), random_base64(32)?));
        params.push(("dm_cover_img_str".into(), random_base64(64)?));
        let mixin = wbi::fetch_mixin()?;
        let signed = wbi::wbi_sign(&params, &mixin);
        net::get_with_headers(
            &format!("{}?{}", endpoint, net::query_string(&signed)),
            cookies,
            &net::RequestHeaders {
                user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/15.2 Safari/605.1.15",
                referer: &format!("https://space.bilibili.com/{mid}"),
                origin: "https://space.bilibili.com",
            },
        )
    })?;
    if value["code"].as_i64() != Some(0) {
        return Err(api_err("用户作品", &value));
    }
    let mut items = Vec::new();
    if let Some(videos) = value["data"]["list"]["vlist"].as_array() {
        for video in videos {
            let bvid = video["bvid"].as_str().unwrap_or_default();
            if bvid.is_empty() {
                continue;
            }
            items.push(Item {
                title: video["title"].as_str().unwrap_or_default().into(),
                meta: video["author"].as_str().unwrap_or_default().into(),
                bvid: bvid.into(),
                pic: video["pic"].as_str().unwrap_or_default().into(),
                duration: video["length"].as_str().unwrap_or_default().into(),
                views: count_text(&video["play"]),
                likes: count_text(&video["like"]),
                danmaku: count_text(&video["video_review"]),
                ..Default::default()
            });
        }
    }
    let has_more = value["data"]["page"]["count"]
        .as_u64()
        .map(|count| u64::from(page) * 20 < count)
        .unwrap_or(items.len() == 20);
    Ok(model::VideoPage {
        items,
        has_more,
        total: None,
    })
}

pub fn related_videos(bvid: &str, cookies: Option<&str>) -> Result<model::VideoPage, String> {
    let query = net::query_string(&[("bvid".into(), bvid.into())]);
    let url = format!("https://api.bilibili.com/x/web-interface/archive/related?{query}");
    let value = cache::api_json(&url, cookies, cache::TTL_LIST)?;
    if value["code"].as_i64() != Some(0) {
        return Err(api_err("相关推荐", &value));
    }
    Ok(model::VideoPage {
        items: video_items(&value["data"]),
        has_more: false,
        total: None,
    })
}

pub fn author_profile(mid: &str, cookies: Option<&str>) -> Result<model::AuthorProfile, String> {
    let query = net::query_string(&[("mid".into(), mid.into()), ("photo".into(), "false".into())]);
    let url = format!("https://api.bilibili.com/x/web-interface/card?{query}");
    let value = cache::api_json(&url, cookies, cache::TTL_DETAIL)?;
    if value["code"].as_i64() != Some(0) {
        return Err(api_err("作者资料", &value));
    }
    let card = &value["data"]["card"];
    Ok(model::AuthorProfile {
        name: card["name"].as_str().unwrap_or_default().into(),
        face: card["face"].as_str().unwrap_or_default().into(),
        level: card["level_info"]["current_level"]
            .as_i64()
            .unwrap_or_default(),
        followers: value["data"]["follower"].as_i64().unwrap_or_default(),
    })
}

// 全部关注分区的哨兵 tagid，与 B 站真实分组 id 区分
pub const ALL_FOLLOWING_TAGID: i64 = i64::MIN;

pub struct FollowingTag {
    pub tagid: i64,
    pub name: String,
    pub count: u64,
}

pub fn following_tags(cookies: &str) -> Result<Vec<FollowingTag>, String> {
    let value = cache::api_json(
        "https://api.bilibili.com/x/relation/tags",
        Some(cookies),
        cache::TTL_LIST,
    )?;
    if value["code"].as_i64() != Some(0) {
        return Err(api_err("关注分组", &value));
    }
    Ok(value["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|tag| {
            Some(FollowingTag {
                tagid: tag["tagid"].as_i64()?,
                name: tag["name"].as_str().unwrap_or_default().into(),
                count: tag["count"].as_u64().unwrap_or(0),
            })
        })
        .collect())
}

// 从 B 站用户 JSON 构造关注项，两个关注列表接口共用
fn parse_following(user: &serde_json::Value) -> Option<model::Following> {
    Some(model::Following {
        owner: Owner {
            mid: user["mid"].as_i64()?.to_string(),
            name: user["uname"].as_str().unwrap_or_default().into(),
        },
        face: user["face"].as_str().unwrap_or_default().into(),
        signature: user["sign"].as_str().unwrap_or_default().replace(['\n', '\r'], " "),
    })
}

pub fn tag_followings(
    tagid: i64,
    page: u32,
    cookies: &str,
) -> Result<model::FollowingPage, String> {
    let params = net::query_string(&[
        ("tagid".into(), tagid.to_string()),
        ("pn".into(), page.to_string()),
        ("ps".into(), "20".into()),
        ("order".into(), "desc".into()),
    ]);
    let value = cache::api_json(
        &format!("https://api.bilibili.com/x/relation/tag?{params}"),
        Some(cookies),
        cache::TTL_LIST,
    )?;
    if value["code"].as_i64() != Some(0) {
        return Err(api_err("关注分组", &value));
    }
    let items = value["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(parse_following)
        .collect::<Vec<_>>();
    let has_more = items.len() == 20;
    Ok(model::FollowingPage { items, has_more })
}

pub fn followings(mid: &str, page: u32, cookies: &str) -> Result<model::FollowingPage, String> {
    let params = net::query_string(&[
        ("vmid".into(), mid.into()),
        ("pn".into(), page.to_string()),
        ("ps".into(), "20".into()),
        ("order".into(), "desc".into()),
        ("order_type".into(), String::new()),
    ]);
    let value = cache::api_json(
        &format!("https://api.bilibili.com/x/relation/followings?{params}"),
        Some(cookies),
        cache::TTL_LIST,
    )?;
    if value["code"].as_i64() != Some(0) {
        return Err(api_err("我的关注", &value));
    }
    let items = value["data"]["list"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(parse_following)
        .collect::<Vec<_>>();
    let has_more = !items.is_empty()
        && value["data"]["total"]
            .as_u64()
            .map(|total| u64::from(page) * 20 < total)
            .unwrap_or(items.len() == 20);
    Ok(model::FollowingPage { items, has_more })
}

pub const WATCH_LATER_URL: &str = "https://api.bilibili.com/x/v2/history/toview";

pub fn watch_later(cookies: &str) -> Result<model::VideoPage, String> {
    let v = cache::api_json(WATCH_LATER_URL, Some(cookies), cache::TTL_LIST)?;
    if v["code"].as_i64() != Some(0) {
        return Err(api_err("稍后再看", &v));
    }
    let mut items = Vec::new();
    if let Some(list) = v["data"]["list"].as_array() {
        for it in list {
            let bvid = it["bvid"].as_str().unwrap_or_default();
            if bvid.is_empty() {
                continue;
            }
            let lt = it["long_title"].as_str().unwrap_or("");
            let title = if !lt.is_empty() {
                lt.to_string()
            } else {
                it["title"].as_str().unwrap_or("").to_string()
            };
            let d = model::dur_of(&it["duration"]);
            let pic = it["pic"].as_str().or_else(|| it["cover"].as_str()).unwrap_or("");
            let author = it["owner"]["name"]
                .as_str()
                .or_else(|| it["author_name"].as_str())
                .unwrap_or("");
            let add_at = it["add_at"].as_i64().unwrap_or_default();
            items.push(Item {
                title,
                meta: author.into(),
                bvid: bvid.into(),
                pic: pic.into(),
                duration: model::mmss(d),
                views: count_text(&it["stat"]["view"]),
                danmaku: count_text(&it["stat"]["danmaku"]),
                progress: it["progress"].as_i64().unwrap_or_default(),
                view_at: add_at,
                date: Some(model::date_of(add_at)),
                duration_secs: d,
                ..Default::default()
            });
        }
    }
    Ok(model::VideoPage {
        items,
        has_more: false,
        total: None,
    })
}

#[cfg(test)]
mod tests;
