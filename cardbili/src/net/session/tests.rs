use super::*;

#[test]
fn cookies_preserve_equals_in_values_and_last_duplicate_wins() {
    let parsed = parse_cookies(" a=1; token=a=b==; malformed; a=2; empty= ");
    assert_eq!(parsed.get("a").map(String::as_str), Some("2"));
    assert_eq!(parsed.get("token").map(String::as_str), Some("a=b=="));
    assert_eq!(parsed.get("empty").map(String::as_str), Some(""));
    assert!(!parsed.contains_key("malformed"));
}

#[test]
fn cookie_header_round_trips_in_stable_order() {
    let parsed = parse_cookies("z=3; a=1; token=a=b");
    let header = cookie_header(&parsed);
    assert_eq!(header, "a=1; token=a=b; z=3");
    assert_eq!(parse_cookies(&header), parsed);
    assert_eq!(cookie_header(&BTreeMap::new()), "");
}

#[test]
fn cookie_lookup_requires_exact_name() {
    assert_eq!(
        cookie("not_bili_jct=wrong; bili_jct=dummy", "bili_jct").as_deref(),
        Some("dummy")
    );
    assert_eq!(cookie("bili_jct_extra=wrong", "bili_jct"), None);
}
