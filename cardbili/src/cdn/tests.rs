use super::*;
use serde_json::json;

fn select(id: &str, value: Value) -> Result<String, String> {
    OPTIONS[index(id)].url(&value)
}

#[test]
fn unknown_cdn_falls_back_to_backup() {
    assert_eq!(OPTIONS[index("unknown")].id, "backup");
    for (i, cdn) in OPTIONS.iter().enumerate() {
        assert_eq!(index(cdn.id), i);
    }
}

#[test]
fn base_uses_first_nonempty_address() {
    assert_eq!(
        select(
            "base",
            json!({"baseUrl": "https://base/video", "backupUrl": ["https://backup/video"]})
        )
        .unwrap(),
        "https://base/video"
    );
    assert_eq!(
        select(
            "base",
            json!({"baseUrl": "", "backupUrl": [null, "", "https://backup/video"]})
        )
        .unwrap(),
        "https://backup/video"
    );
    assert!(select("base", json!({})).is_err());
}

#[test]
fn backup_prefers_replaceable_mirror_and_excludes_mcdn() {
    let mirror = "https://upos-sz-mirrorcos.bilivideo.com/upgcxcode/file.m4s?token=abc";
    let value = json!({"baseUrl": "https://p2p.example/video", "backupUrl": [
        "https://mcdn.bilivideo.com/upgcxcode/file.m4s?os=mcdn&token=x",
        "https://unrelated.example/upgcxcode/file.m4s", mirror
    ]});
    assert_eq!(select("backup", value).unwrap(), mirror);
    assert_eq!(
        select("backup", json!({"baseUrl": "https://p2p.example/video"})).unwrap(),
        "https://p2p.example/video"
    );
}

#[test]
fn selected_mirror_keeps_api_provided_signature() {
    let url = "https://upos-sz-mirrorali.bilivideo.com/upgcxcode/video?signature=original";
    assert_eq!(
        select(
            "ali",
            json!({"baseUrl": "https://other/video", "backupUrl": [url]})
        )
        .unwrap(),
        url
    );
}

#[test]
fn mirror_replacement_preserves_path_and_query() {
    let value = json!({"baseUrl": "http://upos-sz-mirrorcos.bilivideo.com/upgcxcode/1/2/video.m4s?deadline=12&sign=a%2Fb"});
    assert_eq!(
        select("ali", value).unwrap(),
        "https://upos-sz-mirrorali.bilivideo.com/upgcxcode/1/2/video.m4s?deadline=12&sign=a%2Fb"
    );
    assert!(select("ali", json!({"baseUrl": "https://p2p.example/video"})).is_err());
}

#[test]
fn akamai_requires_its_own_signed_address() {
    assert!(select(
        "akamai",
        json!({"baseUrl": "https://mirror.bilivideo.com/upgcxcode/video"})
    )
    .is_err());
    let url = "https://upos-hz-mirrorakam.akamaized.net/video?token=unique";
    assert_eq!(select("akamai", json!({"backupUrl": [url]})).unwrap(), url);
}
