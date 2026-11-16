use super::*;
use serde_json::json;

#[test]
fn xml_decodes_named_and_unicode_numeric_entities_once() {
    assert_eq!(unescape_xml("&lt;&gt;&quot;&apos;&amp;&nbsp;"), "<>\"'& ");
    assert_eq!(unescape_xml("&#20013;&#x6587;&#X1F600;"), "中文😀");
    assert_eq!(unescape_xml("&amp;lt;"), "&lt;");
}

#[test]
fn xml_preserves_invalid_entities_and_plain_unicode() {
    for text in [
        "普通文本😀",
        "&unknown;",
        "&#xD800;",
        "&#1114112;",
        "&unfinished",
        "a&b",
        "&#xZZ;",
    ] {
        assert_eq!(unescape_xml(text), text);
    }
    assert_eq!(unescape_xml("&bad;后面&lt;"), "&bad;后面<");
}

#[test]
fn danmaku_xml_parses_multiple_entries_and_skips_bad_parameters() {
    let xml = r#"<i><d p="1.5,1,25,16711680,0">你好&amp;世界</d>
        <d p="bad,1,25,1">无效时间</d><d p="2,5,25,bad">顶部</d></i>"#;
    let items = parse_danmaku(xml);
    assert_eq!(items.len(), 2);
    assert_eq!(
        (items[0].t, items[0].mode, items[0].color),
        (1.5, 1, 0xff0000)
    );
    assert_eq!(items[0].text, "你好&世界");
    assert_eq!(
        (items[1].t, items[1].mode, items[1].color),
        (2.0, 5, 0xffffff)
    );
}

#[test]
fn danmaku_xml_keeps_complete_entries_before_truncated_input() {
    for tail in [r#"<d p="1,1"#, r#"<d p="1,1,25,1">unfinished"#] {
        let xml = format!(r#"<d p="0,1,25,1">完整</d>{tail}"#);
        let items = parse_danmaku(&xml);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].text, "完整");
    }
    assert!(parse_danmaku("").is_empty());
    assert!(parse_params("1,300", "text".into()).is_none());
}

#[test]
fn video_items_skip_missing_ids_and_accept_mixed_stat_types() {
    let items = video_items(&json!([
        {"title": "missing id"}, {"bvid": ""},
        {"bvid": "BV-test", "title": "标题", "owner": {"name": "作者"},
         "duration": "61.5", "pic": "cover.png",
         "stat": {"view": 12345, "like": "--", "danmaku": 0}},
        {"bvid": "BV-minimal"}
    ]));
    assert_eq!(items.len(), 2);
    let item = &items[0];
    assert_eq!(
        (&*item.bvid, &*item.title, &*item.meta),
        ("BV-test", "标题", "作者")
    );
    assert_eq!((&*item.duration, &*item.pic), ("1:01", "cover.png"));
    assert_eq!(item.views.as_deref(), Some("1.2万"));
    assert_eq!(item.likes.as_deref(), Some("--"));
    assert_eq!(item.danmaku.as_deref(), Some("0"));
    assert_eq!(items[1].duration, "0:00");
    assert!(items[1].views.is_none());
    assert!(video_items(&json!(null)).is_empty());
}

#[test]
fn count_text_distinguishes_zero_from_missing_values() {
    assert_eq!(count_text(&json!(0)).as_deref(), Some("0"));
    assert_eq!(count_text(&json!("1.2万")).as_deref(), Some("1.2万"));
    for value in [json!(null), json!(""), json!(false), json!({})] {
        assert_eq!(count_text(&value), None);
    }
}

#[test]
fn api_errors_preserve_business_code_and_message() {
    assert_eq!(
        api_err("视频", &json!({"code": -101, "message": "未登录"})),
        "视频加载失败\ncode=-101 未登录"
    );
    assert_eq!(api_err("视频", &json!({})), "视频加载失败\ncode=-1");
}
