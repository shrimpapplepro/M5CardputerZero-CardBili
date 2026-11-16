#[cfg(not(feature = "desktop"))]
use std::fs::File;
#[cfg(not(feature = "desktop"))]
use std::io::Read;
#[cfg(not(feature = "desktop"))]
use std::sync::mpsc;
#[cfg(not(feature = "desktop"))]
use std::thread;

use slint::Rgb8Pixel;

pub fn key_release(name: &str) -> Option<&'static str> {
    match name {
        "l" | "5" => Some("LIKE_UP"),
        "7" => Some("7_UP"),
        "8" => Some("8_UP"),
        "LEFT" | "z" => Some("LEFT_UP"),
        "RIGHT" | "c" => Some("RIGHT_UP"),
        "UP" | "f" | "w" => Some("UP_UP"),
        "DOWN" | "x" | "s" => Some("DOWN_UP"),
        _ => None,
    }
}

#[cfg(not(feature = "desktop"))]
#[rustfmt::skip]
pub fn code_name(code: u16) -> Option<&'static str> {
    Some(match code {
        103 => "UP", 108 => "DOWN", 105 => "LEFT", 106 => "RIGHT",
        104 => "PGUP", 109 => "PGDN",
        28 => "ENTER", 57 => "SPACE", 1 => "ESC", 14 => "BACKSPACE", 15 => "TAB",
        16 => "q", 17 => "w", 18 => "e", 19 => "r", 20 => "t", 21 => "y", 22 => "u",
        23 => "i", 24 => "o", 25 => "p", 30 => "a", 31 => "s", 32 => "d", 33 => "f",
        34 => "g", 35 => "h", 36 => "j", 37 => "k", 38 => "l", 44 => "z", 45 => "x",
        46 => "c", 47 => "v", 48 => "b", 49 => "n", 50 => "m",
        2 => "1", 3 => "2", 4 => "3", 5 => "4", 6 => "5", 7 => "6", 8 => "7", 9 => "8", 10 => "9", 11 => "0",
        26 => "1", 27 => "2", 39 => "3", 40 => "4", 41 => "5", 43 => "6", 51 => "7", 52 => "8", 53 => "9", 94 => "0",
        _ => return None,
    })
}

#[cfg(not(feature = "desktop"))]
pub fn spawn_keys(path: String, tx: mpsc::Sender<String>) {
    thread::spawn(move || {
        let mut f = match File::open(&path) {
            Ok(f) => f,
            Err(_) => return,
        };
        let mut buf = [0u8; 24];
        let mut esc_down: Option<std::time::Instant> = None;
        let mut shift = [false; 2];
        loop {
            if f.read_exact(&mut buf).is_err() {
                break;
            }
            let ty = u16::from_le_bytes([buf[16], buf[17]]);
            let code = u16::from_le_bytes([buf[18], buf[19]]);
            let value = i32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);
            if ty != 1 {
                continue;
            }
            if code == 42 || code == 54 {
                shift[usize::from(code == 54)] = value != 0;
                continue;
            }
            // ESC 单独处理：短按返回，长按退出
            if code == 1 {
                if value == 1 {
                    esc_down = Some(std::time::Instant::now());
                    if tx.send("ESC_DOWN".to_string()).is_err() {
                        break;
                    }
                } else if value == 0 {
                    let held = esc_down
                        .take()
                        .map(|t| t.elapsed().as_millis())
                        .unwrap_or(0);
                    if tx.send("ESC_UP".to_string()).is_err() {
                        break;
                    }
                    let name = if held >= 2000 { "ESC_HOLD" } else { "ESC" };
                    if tx.send(name.to_string()).is_err() {
                        break;
                    }
                }
                continue;
            }
            if value == 1 {
                if let Some(n) = code_name(code) {
                    let n = if n == "TAB" && shift.iter().any(|down| *down) {
                        "BACKTAB"
                    } else {
                        n
                    };
                    if tx.send(n.to_string()).is_err() {
                        break;
                    }
                }
            } else if value == 0 {
                // 方向键松开事件，供长按连续调整进度使用
                if let Some(release) = code_name(code).and_then(key_release) {
                    if tx.send(release.to_string()).is_err() {
                        break;
                    }
                }
            }
        }
    });
}

pub fn raw_to_image(w: u32, h: u32, raw: &[u8]) -> slint::Image {
    let mut pb = slint::SharedPixelBuffer::<Rgb8Pixel>::new(w, h);
    for (pixel, channels) in pb.make_mut_slice().iter_mut().zip(raw.chunks_exact(3)) {
        *pixel = Rgb8Pixel {
            r: channels[0],
            g: channels[1],
            b: channels[2],
        };
    }
    slint::Image::from_rgb8(pb)
}

pub fn make_placeholder() -> slint::Image {
    let mut pb = slint::SharedPixelBuffer::<Rgb8Pixel>::new(2, 2);
    for p in pb.make_mut_slice() {
        *p = Rgb8Pixel {
            r: 26,
            g: 33,
            b: 46,
        };
    }
    slint::Image::from_rgb8(pb)
}

pub fn fetch_cover(pic: &str) -> Option<(u32, u32, Vec<u8>)> {
    crate::cache::cover(pic)
}

pub fn make_qr(url: &str) -> Option<slint::Image> {
    use qrcodegen::{QrCode, QrCodeEcc};
    let code = QrCode::encode_text(url, QrCodeEcc::Low).ok()?;
    let n = code.size();
    let quiet = 4usize;
    let total = n as usize + 2 * quiet;
    let scale = (118usize / total).max(1);
    let dim = (total * scale) as u32;
    let mut pb = slint::SharedPixelBuffer::<Rgb8Pixel>::new(dim, dim);
    {
        let sl = pb.make_mut_slice();
        for p in sl.iter_mut() {
            *p = Rgb8Pixel {
                r: 255,
                g: 255,
                b: 255,
            };
        }
        for y in 0..n {
            for x in 0..n {
                if code.get_module(x, y) {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            let px = (x as usize + quiet) * scale + dx;
                            let py = (y as usize + quiet) * scale + dy;
                            if px < dim as usize && py < dim as usize {
                                sl[py * dim as usize + px] = Rgb8Pixel { r: 0, g: 0, b: 0 };
                            }
                        }
                    }
                }
            }
        }
    }
    Some(slint::Image::from_rgb8(pb))
}

pub fn parse_mid(cookies: &str) -> String {
    for kv in cookies.split(';') {
        let kv = kv.trim();
        if let Some(v) = kv.strip_prefix("DedeUserID=") {
            if !v.is_empty() {
                return v.to_string();
            }
        }
    }
    String::new()
}

#[cfg(not(windows))]
pub fn state_dir() -> std::path::PathBuf {
    app_dir("XDG_STATE_HOME", ".local/state", "/var/tmp")
}

#[cfg(not(windows))]
pub fn config_dir() -> std::path::PathBuf {
    app_dir("XDG_CONFIG_HOME", ".config", "/tmp")
}

#[cfg(not(windows))]
pub fn cache_dir() -> std::path::PathBuf {
    app_dir("XDG_CACHE_HOME", ".cache", "/var/tmp")
}

#[cfg(windows)]
pub fn state_dir() -> std::path::PathBuf {
    windows_dir("LOCALAPPDATA", "AppData\\Local")
}

#[cfg(windows)]
pub fn config_dir() -> std::path::PathBuf {
    windows_dir("APPDATA", "AppData\\Roaming")
}

#[cfg(windows)]
pub fn cache_dir() -> std::path::PathBuf {
    state_dir().join("cache")
}

// Windows 使用 LOCALAPPDATA / APPDATA，缺失时回退到 USERPROFILE 下的默认位置
#[cfg(windows)]
fn windows_dir(variable: &str, fallback: &str) -> std::path::PathBuf {
    std::env::var_os(variable)
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(|home| std::path::PathBuf::from(home).join(fallback))
        })
        .unwrap_or_else(|| std::path::PathBuf::from(fallback))
        .join("cardbili")
}

// 优先读取 XDG 环境变量，仅接受绝对路径，未设置时回退到 HOME 下的默认位置
#[cfg(not(windows))]
fn app_dir(variable: &str, default: &str, fallback_home: &str) -> std::path::PathBuf {
    std::env::var_os(variable)
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| {
            std::path::PathBuf::from(
                std::env::var_os("HOME").unwrap_or_else(|| fallback_home.into()),
            )
            .join(default)
        })
        .join("cardbili")
}

/// 估算单行文字宽度，ASCII 按字号的 0.55 倍计算，表情含字距占 14px，其余字符按字号计算
pub fn est_w(s: &str, fs: f64) -> f64 {
    crate::emoji::tokens(s)
        .map(|token| match token {
            crate::emoji::Token::Emoji(_) => f64::from(crate::emoji::ADVANCE),
            crate::emoji::Token::Character(character) => {
                if character.is_ascii() {
                    fs * 0.55
                } else {
                    fs
                }
            }
        })
        .sum()
}

#[cfg(test)]
mod tests;
