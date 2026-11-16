use crate::{diagnostics, net};
use serde_json::Value;
use std::io::Read;
use std::time::{Duration, Instant};

pub struct Cdn {
    pub id: &'static str,
    pub name: &'static str,
    host: &'static str,
}

// 线路参考 PiliPlus 的 CDNService
pub const OPTIONS: &[Cdn] = &[
    Cdn {
        id: "base",
        name: "基础地址",
        host: "",
    },
    Cdn {
        id: "backup",
        name: "备用地址（默认）",
        host: "",
    },
    Cdn {
        id: "ali",
        name: "阿里云 ali",
        host: "upos-sz-mirrorali.bilivideo.com",
    },
    Cdn {
        id: "alib",
        name: "阿里云 alib",
        host: "upos-sz-mirroralib.bilivideo.com",
    },
    Cdn {
        id: "alio1",
        name: "阿里云 alio1",
        host: "upos-sz-mirroralio1.bilivideo.com",
    },
    Cdn {
        id: "cos",
        name: "腾讯云 cos",
        host: "upos-sz-mirrorcos.bilivideo.com",
    },
    Cdn {
        id: "cosb",
        name: "腾讯云 cosb",
        host: "upos-sz-mirrorcosb.bilivideo.com",
    },
    Cdn {
        id: "coso1",
        name: "腾讯云 coso1",
        host: "upos-sz-mirrorcoso1.bilivideo.com",
    },
    Cdn {
        id: "hw",
        name: "华为云 hw",
        host: "upos-sz-mirrorhw.bilivideo.com",
    },
    Cdn {
        id: "hwb",
        name: "华为云 hwb",
        host: "upos-sz-mirrorhwb.bilivideo.com",
    },
    Cdn {
        id: "hwo1",
        name: "华为云 hwo1",
        host: "upos-sz-mirrorhwo1.bilivideo.com",
    },
    Cdn {
        id: "08c",
        name: "华为云 08c",
        host: "upos-sz-mirror08c.bilivideo.com",
    },
    Cdn {
        id: "08h",
        name: "华为云 08h",
        host: "upos-sz-mirror08h.bilivideo.com",
    },
    Cdn {
        id: "08ct",
        name: "华为云 08ct",
        host: "upos-sz-mirror08ct.bilivideo.com",
    },
    Cdn {
        id: "tf_hw",
        name: "华为云 tf_hw",
        host: "upos-tf-all-hw.bilivideo.com",
    },
    Cdn {
        id: "tf_tx",
        name: "腾讯云 tf_tx",
        host: "upos-tf-all-tx.bilivideo.com",
    },
    Cdn {
        id: "akamai",
        name: "Akamai 海外",
        host: "upos-hz-mirrorakam.akamaized.net",
    },
    Cdn {
        id: "aliov",
        name: "阿里云海外 aliov",
        host: "upos-sz-mirroraliov.bilivideo.com",
    },
    Cdn {
        id: "cosov",
        name: "腾讯云海外 cosov",
        host: "upos-sz-mirrorcosov.bilivideo.com",
    },
    Cdn {
        id: "hwov",
        name: "华为云海外 hwov",
        host: "upos-sz-mirrorhwov.bilivideo.com",
    },
    Cdn {
        id: "hk_bcache",
        name: "哔哩哔哩香港",
        host: "cn-hk-eq-bcache-01.bilivideo.com",
    },
];

pub fn index(id: &str) -> usize {
    OPTIONS.iter().position(|cdn| cdn.id == id).unwrap_or(1)
}

pub fn host(url: &str) -> &str {
    url.split('/').nth(2).unwrap_or_default()
}

impl Cdn {
    pub fn url(&self, stream: &Value) -> Result<String, String> {
        let urls: Vec<_> = stream["baseUrl"]
            .as_str()
            .into_iter()
            .chain(
                stream["backupUrl"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str),
            )
            .filter(|url| !url.is_empty())
            .collect();
        let first = *urls.first().ok_or("无可用播放地址")?;
        if self.id == "base" {
            return Ok(first.into());
        }
        if !self.host.is_empty() {
            if let Some(url) = urls.iter().find(|url| host(url) == self.host) {
                return Ok((*url).into());
            }
        }
        // Akamai 使用接口提供的独立签名地址
        if self.id == "akamai" {
            return Err("该视频暂无 Akamai 线路".into());
        }
        let mirror = urls.iter().find(|url| {
            host(url).ends_with(".bilivideo.com")
                && url.contains("/upgcxcode/")
                && !url.split(['?', '&']).any(|part| part == "os=mcdn")
        });
        if self.id == "backup" {
            return Ok(mirror.copied().unwrap_or(first).into());
        }
        let source = mirror.ok_or("该视频暂无可替换的 CDN 地址")?;
        let (_, path) = source.split_once("://").unwrap().1.split_once('/').unwrap();
        Ok(format!("https://{}/{path}", self.host))
    }

    pub fn measure(&self, stream: &Value) -> String {
        let result = self.url(stream).and_then(|url| {
            diagnostics::write(format_args!(
                "[CDN] 测速开始 线路={} 主机={}",
                self.id,
                host(&url)
            ));
            const LIMIT: u64 = 524288;
            let started = Instant::now();
            let measured = (|| -> Result<_, ureq::Error> {
                let mut response = net::new_agent()
                    .get(&url)
                    .header("Referer", net::REFERER)
                    .header("Range", "bytes=0-524287")
                    .config()
                    .timeout_connect(Some(Duration::from_secs(3)))
                    .timeout_global(Some(Duration::from_secs(5)))
                    .build()
                    .call()?;
                if !matches!(response.status().as_u16(), 200 | 206) {
                    return Err(ureq::Error::StatusCode(response.status().as_u16()));
                }
                if response
                    .body()
                    .content_length()
                    .is_some_and(|size| size > LIMIT)
                {
                    return Err(ureq::Error::BodyExceedsLimit(LIMIT));
                }
                let mut reader = response.body_mut().as_reader();
                let mut buffer = [0; 8192];
                let first = reader.read(&mut buffer)? as u64;
                let first_byte = started.elapsed();
                let remaining =
                    std::io::copy(&mut reader.take(LIMIT + 1 - first), &mut std::io::sink())?;
                let bytes = first + remaining;
                if bytes > LIMIT {
                    return Err(ureq::Error::BodyExceedsLimit(LIMIT));
                }
                Ok((bytes, first_byte))
            })()
            .map_err(|error| match error {
                ureq::Error::StatusCode(status) => format!("不可用 · HTTP {status}"),
                ureq::Error::Timeout(_) => "测速超时".into(),
                ureq::Error::HostNotFound => "域名解析失败".into(),
                ureq::Error::BodyExceedsLimit(_) => "不支持分段测速".into(),
                _ => "测速失败".into(),
            })?;
            if measured.0 == 0 {
                return Err("未收到视频数据".into());
            }
            Ok(format!(
                "{:.2} MB/s · 首字节 {:.0}ms",
                measured.0 as f64 / started.elapsed().as_secs_f64() / 1_000_000.0,
                measured.1.as_secs_f64() * 1000.0
            ))
        });
        let text = result.unwrap_or_else(|error| error);
        diagnostics::write(format_args!("[CDN] 测速完成 线路={} 结果={text}", self.id));
        text
    }
}

#[cfg(test)]
mod tests;
