pub mod session;

use crate::diagnostics;
use base64::Engine;
use std::io::Read;
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const UA: &str = "Mozilla/5.0 (X11; Linux aarch64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
pub const API_UA: &str = concat!("CardBili/", env!("CARGO_PKG_VERSION"));
pub const WEB_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/15.2 Safari/605.1.15";
pub const REFERER: &str = "https://www.bilibili.com/";

pub struct RequestHeaders<'a> {
    pub user_agent: &'a str,
    pub referer: &'a str,
    pub origin: &'a str,
}

const DEFAULT_HEADERS: RequestHeaders<'static> = RequestHeaders {
    user_agent: API_UA,
    referer: REFERER,
    origin: "https://www.bilibili.com",
};

pub fn new_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .user_agent(UA)
        .timeout_global(Some(Duration::from_secs(25)))
        .http_status_as_error(false)
        .build()
        .into()
}

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(new_agent)
}

pub fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|s| s.as_secs() as i64)
        .unwrap_or(0)
}

fn request(
    client: &ureq::Agent,
    url: &str,
    cookies: Option<&str>,
    headers: &RequestHeaders<'_>,
    body: Option<(&str, &str)>,
    timeout: u32,
) -> Result<Vec<u8>, String> {
    let endpoint = url.split(['?', '#']).next().unwrap_or(url);
    let method = if body.is_some() { "POST" } else { "GET" };
    let started = Instant::now();
    diagnostics::write(format_args!("[请求] {method} {endpoint} 开始"));
    let result = (|| -> Result<_, ureq::Error> {
        let uri = url
            .parse::<ureq::http::Uri>()
            .map_err(|_| ureq::Error::BadUri("invalid request URL".into()))?;
        let bilibili = uri
            .host()
            .is_some_and(|host| host == "bilibili.com" || host.ends_with(".bilibili.com"));
        let client = match cookies.filter(|value| !value.is_empty()) {
            Some(cookie) if bilibili => session::client(cookie)?,
            _ => client.clone(),
        };
        let mut request = ureq::http::Request::builder()
            .method(method)
            .uri(url)
            .header("User-Agent", headers.user_agent)
            .header("Referer", headers.referer)
            .header("Origin", headers.origin)
            .header("Accept", "application/json, text/plain, */*");
        if bilibili {
            request = request
                .header("env", "prod")
                .header("app-key", "android64")
                .header("x-bili-aurora-zone", "sh001");
            if let Some(mid) = client
                .cookie_jar_lock()
                .get("bilibili.com", "/", "DedeUserID")
            {
                let mid = mid.value();
                let key = b"ad1va46a7lza";
                let eid: Vec<u8> = mid
                    .bytes()
                    .enumerate()
                    .map(|(i, byte)| byte ^ key[i % key.len()])
                    .collect();
                request = request.header("x-bili-mid", mid).header(
                    "x-bili-aurora-eid",
                    base64::engine::general_purpose::STANDARD_NO_PAD.encode(eid),
                );
            }
        }
        if let Some((_, content_type)) = body {
            request = request.header("Content-Type", content_type);
        }
        let request = request.body(())?;
        let request = client
            .configure_request(request)
            .timeout_global(Some(Duration::from_secs(timeout.into())))
            .build();
        let mut response = match body {
            Some((body, _)) => client.run(request.map(|_| body))?,
            None => client.run(request)?,
        };
        let status = response.status().as_u16();
        if !response.status().is_success() {
            return Err(ureq::Error::StatusCode(status));
        }
        let deflate = response
            .headers()
            .get("Content-Encoding")
            .is_some_and(|value| value == "deflate");
        let mut bytes = response.body_mut().with_config().read_to_vec()?;
        if deflate {
            let mut decoded = Vec::new();
            // HTTP deflate 使用 zlib 封装，弹幕接口返回原始 DEFLATE 数据
            if flate2::read::ZlibDecoder::new(bytes.as_slice())
                .read_to_end(&mut decoded)
                .is_err()
            {
                decoded.clear();
                flate2::read::DeflateDecoder::new(bytes.as_slice())
                    .read_to_end(&mut decoded)
                    .map_err(|error| ureq::Error::Decompress("deflate", error))?;
            }
            bytes = decoded;
        }
        Ok((status, bytes))
    })();
    if let Some(cookies) = cookies {
        session::persist(cookies);
    }
    match result {
        Ok((status, bytes)) => {
            diagnostics::write(format_args!(
                "[请求] {method} {endpoint} 完成 HTTP={status} 字节={} 耗时={}ms",
                bytes.len(),
                started.elapsed().as_millis()
            ));
            Ok(bytes)
        }
        Err(error) => {
            let message = error.to_string().replace(url, endpoint);
            diagnostics::write(format_args!(
                "[请求] {method} {endpoint} 失败 耗时={}ms: {message}",
                started.elapsed().as_millis()
            ));
            Err(format!("网络请求失败\n{message}"))
        }
    }
}

pub fn get_with_headers(
    url: &str,
    cookies: Option<&str>,
    headers: &RequestHeaders<'_>,
) -> Result<String, String> {
    request(agent(), url, cookies, headers, None, 25)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

pub fn post_form_json(
    url: &str,
    cookies: &str,
    referer: &str,
    params: &[(String, String)],
) -> Result<serde_json::Value, String> {
    post_form_json_timeout(url, cookies, referer, params, 25)
}

pub fn post_form_json_timeout(
    url: &str,
    cookies: &str,
    referer: &str,
    params: &[(String, String)],
    timeout: u32,
) -> Result<serde_json::Value, String> {
    let headers = RequestHeaders {
        user_agent: WEB_UA,
        referer,
        origin: "https://www.bilibili.com",
    };
    let body = request(
        agent(),
        url,
        Some(cookies),
        &headers,
        Some((&query_string(params), "application/x-www-form-urlencoded")),
        timeout,
    )?;
    parse_json(url, &String::from_utf8_lossy(&body))
        .map_err(|error| format!("数据解析失败\n{error}"))
}

pub fn get(url: &str, cookies: Option<&str>) -> Result<String, String> {
    get_with_headers(url, cookies, &DEFAULT_HEADERS)
}

pub fn get_bytes(url: &str) -> Result<Vec<u8>, String> {
    request(agent(), url, None, &DEFAULT_HEADERS, None, 25)
}

pub fn get_with_agent(client: &ureq::Agent, url: &str, timeout: u32) -> Result<String, String> {
    request(client, url, None, &DEFAULT_HEADERS, None, timeout)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

pub fn post_query_with_agent(
    client: &ureq::Agent,
    url: &str,
    params: &[(String, String)],
) -> Result<serde_json::Value, String> {
    let url = format!("{url}?{}", query_string(params));
    let body = request(
        client,
        &url,
        None,
        &DEFAULT_HEADERS,
        Some(("", "application/x-www-form-urlencoded")),
        25,
    )?;
    parse_json(&url, &String::from_utf8_lossy(&body))
        .map_err(|error| format!("登录状态解析失败\n{error}"))
}

pub fn post_json_with_agent(
    client: &ureq::Agent,
    url: &str,
    headers: &RequestHeaders<'_>,
    value: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let body = request(
        client,
        url,
        None,
        headers,
        Some((&value.to_string(), "application/json")),
        25,
    )?;
    parse_json(url, &String::from_utf8_lossy(&body))
        .map_err(|error| format!("数据解析失败\n{error}"))
}

pub fn get_json(url: &str, cookies: Option<&str>) -> Result<serde_json::Value, String> {
    let text = get(url, cookies)?;
    parse_json(url, &text).map_err(|error| format!("数据解析失败\n{error}"))
}

pub fn parse_json(url: &str, body: &str) -> Result<serde_json::Value, serde_json::Error> {
    let endpoint = url.split(['?', '#']).next().unwrap_or(url);
    let result = serde_json::from_str::<serde_json::Value>(body);
    match &result {
        Ok(value) => diagnostics::write(format_args!(
            "[接口] {endpoint} 业务码={:?}",
            value["code"].as_i64()
        )),
        Err(error) => diagnostics::write(format_args!("[接口] {endpoint} JSON 解析失败: {error}")),
    }
    result
}

fn pct(s: &str) -> String {
    let mut o = String::new();
    for &b in s.as_bytes() {
        let c = b as char;
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~') {
            o.push(c);
        } else {
            o.push_str(&format!("%{:02X}", b));
        }
    }
    o
}

pub fn query_string(params: &[(String, String)]) -> String {
    params
        .iter()
        .map(|(k, v)| format!("{}={}", pct(k), pct(v)))
        .collect::<Vec<_>>()
        .join("&")
}

#[cfg(test)]
mod tests;
