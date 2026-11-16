use crate::{api, danmaku::Dm, diagnostics, net, super_chat};
use serde_json::{json, Value};
use std::io::{self, Read};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::thread;
use std::time::{Duration, Instant};
use tungstenite::client::IntoClientRequest;
use tungstenite::protocol::WebSocketConfig;
use tungstenite::{client_tls_with_config, Error, Message};

const MAX_PACKET: usize = 2 * 1024 * 1024;
const HEARTBEAT: Duration = Duration::from_secs(30);

pub enum Event {
    Danmaku(Dm),
    Audience { text: String, initial: bool },
    Unavailable,
}

pub struct Connection {
    pub events: Receiver<Event>,
    pub super_chats: Receiver<super_chat::Update>,
    stop: Sender<()>,
}

impl Connection {
    pub fn start(room: u64, cookies: Option<String>) -> Self {
        let (tx, events) = mpsc::sync_channel(64);
        let (stop, cancelled) = mpsc::channel();
        let (sc_tx, super_chats) = mpsc::channel();
        let initial_tx = sc_tx.clone();
        let initial_cookies = cookies.clone();
        thread::spawn(move || {
            let result = api::live::super_chat_messages(room, initial_cookies.as_deref());
            let _ = initial_tx.send(super_chat::Update::Initial(result));
        });
        let audience_tx = tx.clone();
        let audience_cookies = cookies.clone();
        thread::spawn(move || match api::live::audience(room, audience_cookies.as_deref()) {
            Ok(text) => {
                let _ = audience_tx.send(Event::Audience {
                    text,
                    initial: true,
                });
            }
            Err(error) => diagnostics::write(format_args!("[直播观众] 房间={room} {error}")),
        });
        thread::spawn(move || {
            let mut delay = 2;
            let mut reported = false;
            loop {
                if cancelled.try_recv().is_ok() {
                    break;
                }
                match receive(room, cookies.as_deref(), &tx, &sc_tx, &cancelled) {
                    Ok(()) => break,
                    Err(error) => {
                        diagnostics::write(format_args!("[直播弹幕] 房间={room} {error}"));
                        if !reported {
                            let _ = tx.try_send(Event::Unavailable);
                            reported = true;
                        }
                    }
                }
                if cancelled.recv_timeout(Duration::from_secs(delay))
                    != Err(mpsc::RecvTimeoutError::Timeout)
                {
                    break;
                }
                delay = (delay * 2).min(30);
            }
        });
        Self {
            events,
            super_chats,
            stop,
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        let _ = self.stop.send(());
    }
}

fn packet(operation: u32, body: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(16 + body.len());
    bytes.extend_from_slice(&(16 + body.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&16u16.to_be_bytes());
    bytes.extend_from_slice(&1u16.to_be_bytes());
    bytes.extend_from_slice(&operation.to_be_bytes());
    bytes.extend_from_slice(&1u32.to_be_bytes());
    bytes.extend_from_slice(body);
    bytes
}

fn receive(
    room: u64,
    cookies: Option<&str>,
    tx: &SyncSender<Event>,
    sc_tx: &Sender<super_chat::Update>,
    cancelled: &Receiver<()>,
) -> Result<(), String> {
    let info = api::live::danmaku_info(room, cookies)?;
    let uid = cookies
        .and_then(|value| net::session::cookie(value, "DedeUserID"))
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let mut auth = json!({
        "uid": uid, "roomid": room, "protover": 3,
        "platform": "web", "type": 2, "key": info.token,
    });
    if let Some(buvid) = cookies.and_then(|value| net::session::cookie(value, "buvid3")) {
        auth["buvid"] = buvid.into();
    }
    let mut socket = None;
    for server in info.servers {
        if cancelled.try_recv().is_ok() {
            return Ok(());
        }
        let addresses = (server.host.as_str(), server.port)
            .to_socket_addrs()
            .map_err(|_| "弹幕服务器域名解析失败")?;
        for address in addresses {
            let Ok(stream) = TcpStream::connect_timeout(&address, Duration::from_secs(5)) else {
                continue;
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .map_err(|e| e.to_string())?;
            stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .map_err(|e| e.to_string())?;
            let mut request = format!("wss://{}:{}/sub", server.host, server.port)
                .into_client_request()
                .map_err(|_| "弹幕连接地址无效")?;
            request
                .headers_mut()
                .insert("Origin", "https://live.bilibili.com".parse().unwrap());
            request
                .headers_mut()
                .insert("User-Agent", net::UA.parse().unwrap());
            let config = WebSocketConfig::default()
                .read_buffer_size(16 * 1024)
                .max_message_size(Some(MAX_PACKET))
                .max_frame_size(Some(MAX_PACKET));
            if let Ok((connected, _)) = client_tls_with_config(request, stream, Some(config), None)
            {
                socket = Some(connected);
                break;
            }
        }
        if socket.is_some() {
            break;
        }
    }
    let mut socket = socket.ok_or("连接弹幕服务器失败")?;
    if cancelled.try_recv().is_ok() {
        return Ok(());
    }
    match socket.get_mut() {
        tungstenite::stream::MaybeTlsStream::Rustls(stream) => stream
            .sock
            .set_read_timeout(Some(Duration::from_millis(500))),
        tungstenite::stream::MaybeTlsStream::Plain(stream) => {
            stream.set_read_timeout(Some(Duration::from_millis(500)))
        }
        _ => return Err("弹幕连接类型无效".into()),
    }
    .map_err(|e| e.to_string())?;
    socket
        .send(Message::binary(packet(7, auth.to_string().as_bytes())))
        .map_err(|_| "发送弹幕认证失败")?;
    let mut authenticated = false;
    let mut received = false;
    let mut last_reply = Instant::now();
    let mut next_heartbeat = Instant::now();
    loop {
        if cancelled.try_recv().is_ok() {
            let _ = socket.close(None);
            return Ok(());
        }
        if last_reply.elapsed()
            > if authenticated {
                HEARTBEAT * 3
            } else {
                Duration::from_secs(10)
            }
        {
            return Err("弹幕服务器响应超时".into());
        }
        if authenticated && Instant::now() >= next_heartbeat {
            socket
                .send(Message::binary(packet(2, &[])))
                .map_err(|_| "发送弹幕心跳失败")?;
            next_heartbeat = Instant::now() + HEARTBEAT;
        }
        match socket.read() {
            Ok(Message::Binary(bytes)) => {
                let mut budget = MAX_PACKET;
                decode(&bytes, 0, &mut budget, &mut |operation, body| {
                    if operation == 3 {
                        last_reply = Instant::now();
                    } else if operation == 8 {
                        let reply: Value =
                            serde_json::from_slice(body).map_err(|_| "弹幕认证响应无效")?;
                        if reply["code"].as_i64() != Some(0) {
                            return Err("弹幕认证失败".into());
                        }
                        authenticated = true;
                        last_reply = Instant::now();
                        diagnostics::write(format_args!("[直播弹幕] 房间={room} 认证成功"));
                    } else if operation == 5 && authenticated {
                        let Ok(value) = serde_json::from_slice::<Value>(body) else {
                            return Ok(());
                        };
                        if let Some(update) = super_chat::parse_update(&value) {
                            let _ = sc_tx.send(update);
                        } else if let Some(text) = parse_audience(&value) {
                            let _ = tx.try_send(Event::Audience {
                                text,
                                initial: false,
                            });
                        } else if let Some(dm) = parse_danmaku(&value) {
                            if !received {
                                diagnostics::write(format_args!(
                                    "[直播弹幕] 房间={room} 首条弹幕已接收"
                                ));
                                received = true;
                            }
                            let _ = tx.try_send(Event::Danmaku(dm));
                        }
                    }
                    Ok(())
                })?;
            }
            Ok(Message::Close(_)) => return Err("弹幕服务器已断开".into()),
            Ok(Message::Ping(_)) => {
                socket.flush().map_err(|_| "弹幕连接已断开")?;
            }
            Ok(_) => {}
            Err(Error::Io(error))
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(_) => return Err("弹幕连接已断开".into()),
        }
    }
}

// 一个 WebSocket 消息可包含多个包，压缩包内仍是完整的协议包
fn decode(
    mut bytes: &[u8],
    depth: u8,
    budget: &mut usize,
    emit: &mut impl FnMut(u32, &[u8]) -> Result<(), String>,
) -> Result<(), String> {
    if depth > 2 || bytes.len() > MAX_PACKET {
        return Err("弹幕数据超出容量限制".into());
    }
    while !bytes.is_empty() {
        if bytes.len() < 16 {
            return Err("弹幕包头不完整".into());
        }
        let size = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
        let header = u16::from_be_bytes(bytes[4..6].try_into().unwrap()) as usize;
        let version = u16::from_be_bytes(bytes[6..8].try_into().unwrap());
        let operation = u32::from_be_bytes(bytes[8..12].try_into().unwrap());
        if header < 16 || size < header || size > bytes.len() {
            return Err("弹幕包长度无效".into());
        }
        let body = &bytes[header..size];
        match version {
            0 | 1 => emit(operation, body)?,
            2 | 3 => {
                let reader: Box<dyn Read + '_> = if version == 2 {
                    Box::new(flate2::read::ZlibDecoder::new(body))
                } else {
                    Box::new(brotli::Decompressor::new(body, 4096))
                };
                let mut decoded = Vec::new();
                reader
                    .take(*budget as u64 + 1)
                    .read_to_end(&mut decoded)
                    .map_err(|_| "弹幕解压失败")?;
                *budget = budget
                    .checked_sub(decoded.len())
                    .ok_or("弹幕解压数据超出容量限制")?;
                decode(&decoded, depth + 1, budget, emit)?;
            }
            _ => {}
        }
        bytes = &bytes[size..];
    }
    Ok(())
}

fn parse_audience(value: &Value) -> Option<String> {
    if value["cmd"].as_str()? != "ONLINE_RANK_COUNT" {
        return None;
    }
    value["data"]["online_count_text"]
        .as_str()
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn parse_danmaku(value: &Value) -> Option<Dm> {
    if value["cmd"].as_str()?.split(':').next()? != "DANMU_MSG" {
        return None;
    }
    let info = &value["info"];
    let text: String = info[1].as_str()?.chars().take(120).collect();
    if text.trim().is_empty() {
        return None;
    }
    Some(Dm {
        t: 0.0,
        mode: info[0][1].as_u64().unwrap_or(1) as u8,
        color: info[0][3].as_u64().unwrap_or(0xffffff) as u32 & 0xffffff,
        text,
    })
}

#[cfg(test)]
mod tests;
