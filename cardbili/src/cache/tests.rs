use super::*;
use serde_json::json;

#[test]
fn cache_keys_and_sharded_paths_are_stable() {
    assert_eq!(key_of("hello"), "5d41402abc4b2a76b9719d911017c592");
    let key = key_of("hello");
    assert_eq!(
        cache_path(Path::new("cache"), &key, "json"),
        Path::new("cache")
            .join("5")
            .join("d")
            .join(format!("{key}.json"))
    );
    assert_ne!(key_of("url\nu:1"), key_of("url\nu:2"));
    assert_ne!(key_of("url\na"), key_of("url\nu:1"));
}

#[test]
fn cached_json_accepts_fresh_files_and_rejects_expired_or_invalid_files() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("data.json");
    assert!(read_cached(&path, 60).is_none());
    fs::write(&path, br#"{"code":0,"data":[1]}"#).unwrap();
    assert_eq!(
        read_cached(&path, 60),
        Some(json!({"code": 0, "data": [1]}))
    );
    let file = fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.set_times(
        fs::FileTimes::new().set_modified(UNIX_EPOCH + std::time::Duration::from_secs(1)),
    )
    .unwrap();
    assert!(read_cached(&path, 60).is_none());
    fs::write(&path, b"not json").unwrap();
    assert!(read_cached(&path, 60).is_none());
}

#[test]
fn force_and_preview_flags_are_thread_local() {
    std::thread::spawn(|| {
        set_force(true);
        set_cached_only(true);
        std::thread::spawn(|| {
            assert!(!force_on());
            assert!(!cached_only_on());
        })
        .join()
        .unwrap();
        assert!(force_on());
        assert!(cached_only_on());
        set_force(false);
        set_cached_only(false);
        assert!(!force_on());
        assert!(!cached_only_on());
    })
    .join()
    .unwrap();
}
