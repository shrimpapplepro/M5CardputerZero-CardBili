use crate::net;
use md5::{Digest, Md5};

const MIXIN_TAB: [usize; 32] = [
    46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19, 29,
    28, 14, 39, 12, 38, 41, 13,
];

fn stem(u: &str) -> String {
    let f = u.rsplit('/').next().unwrap_or(u);
    f.split('.').next().unwrap_or(f).to_string()
}

pub fn mixin_key(img: &str, sub: &str) -> String {
    let orig = format!("{}{}", stem(img), stem(sub));
    let bytes = orig.as_bytes();
    MIXIN_TAB
        .iter()
        .filter_map(|&i| bytes.get(i).map(|&b| b as char))
        .collect()
}

pub fn fetch_mixin() -> Result<String, String> {
    let v = net::get_json("https://api.bilibili.com/x/web-interface/nav", None)?;
    let wi = &v["data"]["wbi_img"];
    let img = wi["img_url"]
        .as_str()
        .ok_or("签名信息不完整\n缺少 img_url")?;
    let sub = wi["sub_url"]
        .as_str()
        .ok_or("签名信息不完整\n缺少 sub_url")?;
    Ok(mixin_key(img, sub))
}

/// 对 params 做 WBI 签名，返回带 wts/w_rid 的新参数表
pub fn wbi_sign(params: &[(String, String)], mk: &str) -> Vec<(String, String)> {
    sign_at(params, mk, net::now_secs())
}

fn sign_at(params: &[(String, String)], mk: &str, timestamp: i64) -> Vec<(String, String)> {
    let mut p: Vec<(String, String)> = params.to_vec();
    p.push(("wts".into(), timestamp.to_string()));
    p.sort_by(|a, b| a.0.cmp(&b.0));
    let filtered: Vec<_> = p
        .iter()
        .map(|(key, value)| {
            let value = value.chars().filter(|c| !"!'()*".contains(*c)).collect();
            (key.clone(), value)
        })
        .collect();
    let q = net::query_string(&filtered);
    let mut h = Md5::new();
    h.update(q.as_bytes());
    h.update(mk.as_bytes());
    let rid = format!("{:x}", h.finalize());
    p.push(("w_rid".into(), rid));
    p
}

#[cfg(test)]
mod tests;
