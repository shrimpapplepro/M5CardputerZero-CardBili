use super::*;

#[test]
fn percent_encoding_preserves_unreserved_ascii_and_encodes_utf8() {
    assert_eq!(pct("AZaz09-_.~"), "AZaz09-_.~");
    assert_eq!(
        pct("中文 a+b&c=!?"),
        "%E4%B8%AD%E6%96%87%20a%2Bb%26c%3D%21%3F"
    );
    assert_eq!(pct("%2F"), "%252F");
}

#[test]
fn query_string_encodes_both_keys_and_values_in_order() {
    assert_eq!(
        query_string(&[("a b".into(), "x=y".into()), ("empty".into(), "".into())]),
        "a%20b=x%3Dy&empty="
    );
    assert_eq!(query_string(&[]), "");
}

#[test]
fn json_parser_reports_malformed_responses() {
    assert_eq!(
        parse_json("https://example.test/api", r#"{"code":0,"data":[1]}"#).unwrap()["data"][0],
        1
    );
    assert!(parse_json("https://example.test/api?token=dummy", "<html>error</html>").is_err());
}
