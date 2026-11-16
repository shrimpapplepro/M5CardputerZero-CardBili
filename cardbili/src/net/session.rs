mod device;
mod storage;

pub use device::{ensure_buvid, initialize, prepare_anonymous};

use super::new_agent;
use crate::diagnostics;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Mutex;

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Credentials {
    pub cookies: BTreeMap<String, String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
}

pub struct Login {
    pub credentials: Credentials,
    pub client: ureq::Agent,
}

struct Session {
    key: String,
    login: Login,
    initialized: bool,
}

impl Session {
    fn persist(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let cookies = snapshot(&self.login.client);
        if cookies == self.login.credentials.cookies {
            return Ok(());
        }
        let credentials = Credentials {
            cookies,
            ..self.login.credentials.clone()
        };
        storage::save(&credentials)?;
        self.login.credentials = credentials;
        Ok(())
    }
}

static CURRENT: Mutex<Option<Session>> = Mutex::new(None);

fn fill_random(buf: &mut [u8]) -> Result<(), Box<dyn std::error::Error>> {
    getrandom::getrandom(buf)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error.to_string()))?;
    Ok(())
}

fn cookie_header(cookies: &BTreeMap<String, String>) -> String {
    cookies
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("; ")
}

fn parse_cookies(header: &str) -> BTreeMap<String, String> {
    header
        .split(';')
        .filter_map(|pair| pair.trim().split_once('='))
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect()
}

pub fn insert(client: &ureq::Agent, name: &str, value: &str) -> Result<(), ureq::Error> {
    let uri = ureq::http::Uri::from_static("https://www.bilibili.com/");
    let cookie = ureq::Cookie::parse(
        format!("{name}={value}; Domain=.bilibili.com; Path=/; Secure"),
        &uri,
    )
    .map_err(|_| ureq::Error::CookieValue("invalid session cookie"))?;
    client.cookie_jar_lock().insert(cookie, &uri)
}

fn snapshot(client: &ureq::Agent) -> BTreeMap<String, String> {
    let jar = client.cookie_jar_lock();
    jar.iter()
        .filter_map(|cookie| jar.get("bilibili.com", "/", cookie.name()))
        .map(|cookie| (cookie.name().to_string(), cookie.value().to_string()))
        .collect()
}

fn from_credentials(credentials: Credentials) -> Result<Session, ureq::Error> {
    let client = new_agent();
    for (name, value) in &credentials.cookies {
        insert(&client, name, value)?;
    }
    Ok(Session {
        key: cookie_header(&credentials.cookies),
        login: Login {
            credentials,
            client,
        },
        initialized: false,
    })
}

pub fn restore() -> Option<String> {
    let result = (|| -> Result<_, Box<dyn std::error::Error>> {
        let Some(credentials) = storage::load()? else {
            return Ok(None);
        };
        if !credentials.cookies.contains_key("SESSDATA") {
            return Ok(None);
        }
        let session = from_credentials(credentials)?;
        let key = session.key.clone();
        *CURRENT.lock().unwrap() = Some(session);
        Ok(Some(key))
    })();
    match result {
        Ok(cookies) => cookies,
        Err(error) => {
            diagnostics::write(format_args!("[登录] 读取登录态失败: {error}"));
            None
        }
    }
}

pub fn accept(mut login: Login) -> Result<String, String> {
    login.credentials.cookies = snapshot(&login.client);
    let key = cookie_header(&login.credentials.cookies);
    let mut current = CURRENT.lock().unwrap();
    storage::save(&login.credentials).map_err(|error| {
        diagnostics::write(format_args!("[登录] 保存登录态失败: {error}"));
        "登录信息保存失败，请重试".to_string()
    })?;
    *current = Some(Session {
        key: key.clone(),
        login,
        initialized: true,
    });
    Ok(key)
}

pub fn client(cookies: &str) -> Result<ureq::Agent, ureq::Error> {
    let mut current = CURRENT.lock().unwrap();
    if let Some(session) = current.as_mut().filter(|session| session.key == cookies) {
        if !session.initialized {
            session.initialized = true;
            initialize(&session.login.client);
            if let Err(error) = session.persist() {
                diagnostics::write(format_args!("[登录] 保存设备会话失败: {error}"));
            }
        }
        return Ok(session.login.client.clone());
    }
    // 已结束会话的在途请求使用独立客户端
    Ok(from_credentials(Credentials {
        cookies: parse_cookies(cookies),
        ..Default::default()
    })?
    .login
    .client)
}

pub fn persist(cookies: &str) {
    let mut current = CURRENT.lock().unwrap();
    let Some(session) = current.as_mut().filter(|session| session.key == cookies) else {
        return;
    };
    if let Err(error) = session.persist() {
        diagnostics::write(format_args!("[登录] 保存会话 Cookie 失败: {error}"));
    }
}

pub fn cookie(cookies: &str, name: &str) -> Option<String> {
    let current = CURRENT.lock().unwrap();
    if let Some(session) = current.as_ref().filter(|session| session.key == cookies) {
        return session
            .login
            .client
            .cookie_jar_lock()
            .get("bilibili.com", "/", name)
            .map(|cookie| cookie.value().to_string());
    }
    parse_cookies(cookies).remove(name)
}

#[cfg(test)]
mod tests;
