use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, Once};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::{diagnostics, net};

// 接口数据缓存有效期，单位为秒
pub const TTL_LIST: u64 = 60;
pub const TTL_DETAIL: u64 = 120;
pub const TTL_PLAYURL: u64 = 120;
pub const TTL_REPLY: u64 = 120;
pub const TTL_ACCOUNT: u64 = 86400;

pub type Pixels = (u32, u32, Vec<u8>);

// 接口和图片共用 1 GiB 缓存空间
pub const MAX_BYTES: u64 = 1024 * 1024 * 1024;

// 超限后一次清理到 90%，为后续写入预留空间
const LOW_WATER: u64 = MAX_BYTES * 9 / 10;

static IMAGE_DOWNLOAD_PAUSED: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

pub fn pause_images(paused: bool) {
    let mut state = IMAGE_DOWNLOAD_PAUSED.0.lock().unwrap();
    if *state != paused {
        *state = paused;
        IMAGE_DOWNLOAD_PAUSED.1.notify_all();
        diagnostics::write(format_args!("[缓存] 图片下载暂停={paused}"));
    }
}

static INIT: Once = Once::new();
static SIZE: Mutex<Option<u64>> = Mutex::new(None);

static SEQ: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static FORCE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

// 当前线程的强制刷新开关
pub fn set_force(v: bool) {
    FORCE.with(|f| f.set(v));
    diagnostics::write(format_args!("[缓存] 强制刷新={v}"));
}

fn force_on() -> bool {
    FORCE.with(|f| f.get())
}

// 缓存预览阶段可接受的数据最大年龄，单位为秒
const STALE_MAX: u64 = 7 * 86400;

thread_local! {
    static CACHED_ONLY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

// 当前线程读取缓存预览数据的开关
pub fn set_cached_only(v: bool) {
    CACHED_ONLY.with(|f| f.set(v));
    diagnostics::write(format_args!("[缓存] 缓存预览={v}"));
}

fn cached_only_on() -> bool {
    CACHED_ONLY.with(|f| f.get())
}

fn base() -> PathBuf {
    crate::util::cache_dir()
}

fn api_dir() -> PathBuf {
    base().join("api")
}

fn img_dir() -> PathBuf {
    base().join("img")
}

pub fn init() {
    INIT.call_once(|| {
        for directory in [api_dir(), img_dir()] {
            if let Err(error) = fs::create_dir_all(&directory) {
                diagnostics::write(format_args!(
                    "[缓存] 创建目录失败 {}: {error}",
                    directory.display()
                ));
            }
        }
        // 容量统计在后台完成，写入、清理和统计共用容量锁
        std::thread::spawn(|| {
            if let Err(error) = usage() {
                diagnostics::write(format_args!("[缓存] 初始化容量失败: {error}"));
            }
        });
        diagnostics::write(format_args!(
            "[缓存] 目录={} 容量上限={MAX_BYTES}",
            base().display()
        ));
    });
}

// 哈希前两位分别作为目录名，每类缓存共 256 个叶子目录
fn cache_path(directory: &Path, key: &str, extension: &str) -> PathBuf {
    directory
        .join(&key[..1])
        .join(&key[1..2])
        .join(format!("{key}.{extension}"))
}

fn entries() -> std::io::Result<Vec<(PathBuf, fs::Metadata)>> {
    let mut files = Vec::new();
    let mut directories = vec![api_dir(), img_dir()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                directories.push(entry.path());
            } else if kind.is_file() {
                files.push((entry.path(), entry.metadata()?));
            }
        }
    }
    Ok(files)
}

pub fn usage() -> std::io::Result<u64> {
    init();
    let mut size = SIZE.lock().unwrap();
    match *size {
        Some(bytes) => Ok(bytes),
        None => {
            let mut bytes = entries()?.iter().map(|(_, m)| m.len()).sum();
            enforce_limit(&mut bytes)?;
            *size = Some(bytes);
            Ok(bytes)
        }
    }
}

pub fn clear() -> std::io::Result<()> {
    init();
    let mut size = SIZE.lock().unwrap();
    let result = (|| -> std::io::Result<()> {
        for directory in [api_dir(), img_dir()] {
            fs::remove_dir_all(&directory)?;
            fs::create_dir_all(directory)?;
        }
        Ok(())
    })();
    *size = if result.is_ok() { Some(0) } else { None };
    diagnostics::write(format_args!("[缓存] 清理结果={result:?}"));
    result
}

// 缓存键：对原始字符串取 MD5
fn key_of(input: &str) -> String {
    use md5::{Digest, Md5};
    let mut h = Md5::new();
    h.update(input.as_bytes());
    format!("{:x}", h.finalize())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn mtime(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// 原子写入：完整写入临时文件后再重命名
fn write_atomic(path: &Path, data: &[u8]) {
    let mut size = SIZE.lock().unwrap();
    let result = (|| -> std::io::Result<()> {
        let total = match *size {
            Some(bytes) => bytes,
            None => entries()?.iter().map(|(_, m)| m.len()).sum(),
        };
        fs::create_dir_all(path.parent().unwrap())?;
        let previous = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let name = path.file_name().unwrap().to_string_lossy();
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let tmp = path.with_file_name(format!(".{name}.{}.{n}.tmp", std::process::id()));
        if let Err(error) = fs::write(&tmp, data).and_then(|_| fs::rename(&tmp, path)) {
            let _ = fs::remove_file(&tmp);
            return Err(error);
        }
        let mut total = total.saturating_sub(previous) + data.len() as u64;
        enforce_limit(&mut total)?;
        *size = Some(total);
        Ok(())
    })();
    match result {
        Ok(()) => diagnostics::write(format_args!(
            "[缓存] 已写入 {} 字节={}",
            path.display(),
            data.len()
        )),
        Err(error) => {
            *size = None;
            diagnostics::write(format_args!("[缓存] 写入失败 {}: {error}", path.display()));
        }
    }
}

// 图片缓存以最后访问时间决定淘汰顺序
fn touch(path: &Path) {
    if let Ok(f) = fs::OpenOptions::new().write(true).open(path) {
        let _ = f.set_modified(SystemTime::now());
    }
}

// 容量超过上限时，按修改时间淘汰最旧的缓存文件至低水位
fn enforce_limit(total: &mut u64) -> std::io::Result<()> {
    if *total <= MAX_BYTES {
        return Ok(());
    }
    let mut files = entries()?;
    files.sort_by_key(|(_, metadata)| metadata.modified().unwrap_or(UNIX_EPOCH));
    let mut failure = None;
    for (path, metadata) in files {
        if *total <= LOW_WATER {
            break;
        }
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                diagnostics::write(format_args!("[缓存] 淘汰失败 {}: {error}", path.display()));
                failure = Some(error);
                continue;
            }
        }
        *total = total.saturating_sub(metadata.len());
        diagnostics::write(format_args!(
            "[缓存] 淘汰 {} 字节={} 剩余={total}",
            path.display(),
            metadata.len()
        ));
    }
    if *total > LOW_WATER {
        if let Some(error) = failure {
            return Err(error);
        }
    }
    Ok(())
}

// JSON 缓存以写入时间计算有效期
fn read_cached(path: &Path, max_age: u64) -> Option<Value> {
    if now().saturating_sub(mtime(path)) > max_age {
        return None;
    }
    let data = fs::read(path)
        .map_err(|error| {
            diagnostics::write(format_args!("[缓存] 读取失败 {}: {error}", path.display()));
        })
        .ok()?;
    serde_json::from_slice(&data)
        .map_err(|error| {
            diagnostics::write(format_args!(
                "[缓存] JSON 解析失败 {}: {error}",
                path.display()
            ));
        })
        .ok()
}

// 带缓存的接口请求
pub fn api_json(url: &str, cookies: Option<&str>, ttl: u64) -> Result<Value, String> {
    api_json_with(url, cookies, ttl, || net::get(url, cookies))
}

// 缓存键使用业务参数，请求签名在缓存未命中时生成
pub fn api_json_with(
    cache_key: &str,
    cookies: Option<&str>,
    ttl: u64,
    fetch: impl FnOnce() -> Result<String, String>,
) -> Result<Value, String> {
    init();
    // 缓存按账号隔离：登录时以 mid 作为业务标识，避免不同账号共用同一接口缓存
    let tag = match cookies {
        Some(c) if !c.is_empty() => format!("u:{}", crate::util::parse_mid(c)),
        _ => "a".to_string(),
    };
    let path = cache_path(&api_dir(), &key_of(&format!("{cache_key}\n{tag}")), "json");
    let endpoint = cache_key.split(['?', '#']).next().unwrap_or(cache_key);
    // 强制刷新优先于缓存预览，普通请求按有效期读取
    if !force_on() {
        let max_age = if cached_only_on() { STALE_MAX } else { ttl };
        if let Some(v) = read_cached(&path, max_age) {
            diagnostics::write(format_args!(
                "[缓存] 接口命中 {endpoint} {} 有效期={max_age}s",
                path.display()
            ));
            return Ok(v);
        }
        diagnostics::write(format_args!(
            "[缓存] 接口未命中 {endpoint} {} 有效期={max_age}s",
            path.display()
        ));
        if cached_only_on() {
            return Err("缓存未命中".to_string());
        }
    } else {
        diagnostics::write(format_args!(
            "[缓存] 接口强制刷新 {endpoint} {}",
            path.display()
        ));
    }
    let s = fetch()?;
    let v = net::parse_json(cache_key, &s).map_err(|e| {
        format!(
            "数据解析失败（{e}）：{}",
            s.chars().take(200).collect::<String>()
        )
    })?;
    if v["code"].as_i64() == Some(0) {
        write_atomic(&path, s.as_bytes());
    }
    Ok(v)
}

// 带缓存的封面：命中直接返回已缩放像素，未命中下载并回填
pub fn cover(pic: &str) -> Option<Pixels> {
    cached_image(pic, 180, 120, false)
}

pub fn avatar(url: &str) -> Option<Pixels> {
    cached_image(url, 64, 64, false)
}

// 评论表情保留透明通道，按目标尺寸等比缩放
pub fn emote(url: &str, size: u32) -> Option<Pixels> {
    cached_image(url, size, size, true)
}

fn cached_image(pic: &str, width: u32, height: u32, alpha: bool) -> Option<Pixels> {
    init();
    let mut url = pic.to_string();
    if let Some(p) = url.find('@') {
        url.truncate(p);
    }
    if url.starts_with("//") {
        url = format!("https:{}", url);
    } else if url.starts_with("http://") {
        url = url.replacen("http://", "https://", 1);
    }
    if url.is_empty() {
        return None;
    }
    let (key, extension) = if alpha {
        (format!("{url}#emote{width}"), "emote")
    } else {
        (format!("{url}@{width}x{height}"), "raw")
    };
    let path = cache_path(&img_dir(), &key_of(&key), extension);
    let channels = if alpha { 4 } else { 3 };
    if let Ok(data) = fs::read(&path) {
        if data.len() >= 8 {
            let w = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            let h = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
            if w > 0
                && h > 0
                && w <= width
                && h <= height
                && data.len() == 8 + w as usize * h as usize * channels
            {
                touch(&path);
                diagnostics::write(format_args!(
                    "[缓存] 图片命中 {} 尺寸={w}x{h}",
                    path.display()
                ));
                return Some((w, h, data[8..].to_vec()));
            }
        }
    }
    diagnostics::write(format_args!(
        "[缓存] 图片未命中 {} 尺寸={width}x{height}",
        path.display()
    ));
    // 图片 CDN 按目标尺寸的两倍缩放，缓存仍使用原图地址作为标识
    let download_url = if crate::cdn::host(&url).ends_with(".hdslb.com") {
        let format = if alpha { "png" } else { "jpg" };
        format!("{url}@{}w_{}h_0e.{format}", width * 2, height * 2)
    } else {
        url.clone()
    };
    let paused = IMAGE_DOWNLOAD_PAUSED.0.lock().unwrap();
    drop(
        IMAGE_DOWNLOAD_PAUSED
            .1
            .wait_while(paused, |paused| *paused)
            .unwrap(),
    );
    let bytes = net::get_bytes(&download_url).ok()?;
    let img = image::load_from_memory(&bytes)
        .map_err(|error| {
            diagnostics::write(format_args!(
                "[缓存] 图片解码失败 {}: {error}",
                path.display()
            ));
        })
        .ok()?;
    let img = img.thumbnail(width, height);
    let (w, h) = (img.width(), img.height());
    let raw = if alpha {
        img.to_rgba8().into_raw()
    } else {
        img.to_rgb8().into_raw()
    };
    let mut buf = Vec::with_capacity(8 + raw.len());
    buf.extend_from_slice(&w.to_le_bytes());
    buf.extend_from_slice(&h.to_le_bytes());
    buf.extend_from_slice(&raw);
    write_atomic(&path, &buf);
    Some((w, h, raw))
}

pub fn invalidate_api(url: &str, cookies: &str) {
    init();
    let tag = format!("u:{}", crate::util::parse_mid(cookies));
    let path = cache_path(&api_dir(), &key_of(&format!("{url}\n{tag}")), "json");
    let mut size = SIZE.lock().unwrap();
    let bytes = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let result = fs::remove_file(&path);
    if result.is_ok() {
        if let Some(total) = size.as_mut() {
            *total = total.saturating_sub(bytes);
        }
    }
    diagnostics::write(format_args!(
        "[缓存] 失效 {} 结果={result:?}",
        path.display()
    ));
}

#[cfg(test)]
mod tests;
