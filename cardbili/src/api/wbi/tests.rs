use super::*;

#[test]
fn mixin_key_matches_reference_vector() {
    assert_eq!(
        mixin_key(
            "https://i0.hdslb.com/bfs/wbi/7cd084941338484aae1ad9425b84077c.png",
            "https://i0.hdslb.com/bfs/wbi/4932caff0ff746eab6f01bf08b70ac45.png",
        ),
        "ea1db124af3c7062474693fa704f4ff8"
    );
}

#[test]
fn signature_matches_fixed_timestamp_reference_vector() {
    let params = vec![
        ("foo".into(), "114".into()),
        ("bar".into(), "514".into()),
        ("baz".into(), "1919810".into()),
    ];
    let signed = sign_at(&params, "ea1db124af3c7062474693fa704f4ff8", 1702204169);
    assert_eq!(
        net::query_string(&signed),
        "bar=514&baz=1919810&foo=114&wts=1702204169&w_rid=6149fdadf571698ca7e6a567265cd0ee"
    );
    assert_eq!(params[0], ("foo".into(), "114".into()));
}

#[test]
fn signing_filters_reserved_characters_before_hashing() {
    let clean = sign_at(&[("name".into(), "中文".into())], "key", 123);
    let filtered = sign_at(&[("name".into(), "中!'()*文".into())], "key", 123);
    assert_eq!(filtered.last(), clean.last());
}
