use super::*;
use serde_json::json;

fn message(id: u64, start: u64, end: u64) -> Message {
    Message::parse(
        &json!({"id": id, "price": 30, "start_time": start, "end_time": end,
        "user_info": {"uname": "测试用户"}, "message": "你好"}),
    )
    .unwrap()
}

#[test]
fn parse_accepts_string_numbers_and_color_overrides() {
    let value = json!({"id": "42", "price": "100", "start_time": "10", "end_time": "20",
        "message": "你好", "background_bottom_color": "#abcdef",
        "background_color": "#123456", "message_font_color": "#010203"});
    let parsed = Message::parse(&value).unwrap();
    assert_eq!(
        (parsed.id, parsed.price, parsed.start, parsed.end),
        (42, 100, 10, 20)
    );
    assert_eq!(
        (parsed.accent, parsed.background, parsed.foreground),
        (0xabcdef, 0x123456, 0x010203)
    );
    assert_eq!(parsed.user, "");
}

#[test]
fn parse_uses_price_tiers_when_colors_are_missing_or_invalid() {
    for (price, accent, background) in [
        (49, 0x2A60B2, 0xEDF5FF),
        (50, 0x427D9E, 0xDBFFFD),
        (100, 0xE2B52B, 0xFFF1C5),
        (500, 0xE09443, 0xFFEAD2),
        (1000, 0xE54D4D, 0xFFE7E4),
        (2000, 0xAB1A32, 0xFFD8D8),
    ] {
        let parsed = Message::parse(
            &json!({"id": 1, "price": price, "start_time": 0, "end_time": 10,
            "message": "text", "background_color": "#bad", "message_font_color": "123456"}),
        )
        .unwrap();
        assert_eq!(
            (parsed.accent, parsed.background, parsed.foreground),
            (accent, background, 0xffffff)
        );
    }
}

#[test]
fn parse_requires_identifiers_timestamps_price_and_text() {
    let value = json!({"id": 1, "price": 30, "start_time": 10, "end_time": 20, "message": "text"});
    for field in ["id", "price", "start_time", "end_time", "message"] {
        let mut invalid = value.clone();
        invalid.as_object_mut().unwrap().remove(field);
        assert!(Message::parse(&invalid).is_none(), "field={field}");
    }
}

#[test]
fn visibility_includes_start_and_excludes_end() {
    let item = message(1, 10, 20);
    assert!(!item.visible(9));
    assert!(item.visible(10));
    assert!(item.visible(19));
    assert!(!item.visible(20));
    assert_eq!(item.announcement().text, "【¥30】测试用户的醒目留言：你好");
}

#[test]
fn updates_accept_command_suffixes_and_filter_invalid_delete_ids() {
    let update = parse_update(&json!({"cmd": "SUPER_CHAT_MESSAGE_DELETE:extra", "data": {"ids": [1, "2", null, -1, "bad"]}})).unwrap();
    let Update::Delete(ids) = update else {
        panic!("expected deletion")
    };
    assert_eq!(ids, [1, 2]);
    assert!(parse_update(&json!({"cmd": "OTHER"})).is_none());
}

#[test]
fn live_updates_override_late_initial_snapshot() {
    let mut state = State::default();
    let mut latest = message(1, 10, 50);
    latest.text = "更新内容".into();
    assert!(state.apply(Update::Add(latest.clone())));
    state.apply(Update::Delete(vec![2]));
    state.apply(Update::Initial(Ok(vec![
        message(1, 10, 20),
        message(2, 12, 30),
        message(3, 11, 40),
    ])));
    assert!(state.loaded);
    assert!(!state.failed);
    assert_eq!(state.messages, [message(3, 11, 40), latest]);
    assert_eq!(state.announcements.len(), 1);
}

#[test]
fn duplicate_add_replaces_message_without_reannouncing() {
    let mut state = State::default();
    assert!(state.apply(Update::Add(message(1, 10, 20))));
    assert!(!state.apply(Update::Add(message(1, 10, 30))));
    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.messages[0].end, 30);
    assert_eq!(state.announcements.len(), 1);
}

#[test]
fn initial_messages_are_sorted_deduplicated_and_not_announced() {
    let mut state = State::default();
    state.apply(Update::Initial(Ok(vec![
        message(1, 10, 20),
        message(3, 11, 30),
        message(2, 11, 30),
        message(1, 10, 20),
    ])));
    assert_eq!(
        state.messages.iter().map(|m| m.id).collect::<Vec<_>>(),
        [3, 2, 1]
    );
    assert!(state.announcements.is_empty());
}

#[test]
fn initial_failure_preserves_received_live_messages() {
    let mut state = State::default();
    state.apply(Update::Add(message(1, 10, 20)));
    state.apply(Update::Initial(Err("offline".into())));
    assert!(state.loaded && state.failed);
    assert_eq!(state.messages.len(), 1);
}

#[test]
fn expiry_reports_activation_once_and_cleans_announcements() {
    let mut state = State::default();
    state.apply(Update::Add(message(1, 10, 20)));
    assert!(state.expire(10, false));
    assert!(!state.expire(11, false));
    assert_eq!(state.announcements.len(), 1);
    assert!(state.expire(20, false));
    assert!(state.messages.is_empty());
    assert!(state.announcements.is_empty());
}

#[test]
fn open_popup_keeps_expired_messages_until_closed() {
    let mut state = State::default();
    state.apply(Update::Add(message(1, 10, 20)));
    state.expire(10, true);
    assert!(!state.expire(20, true));
    assert_eq!(state.messages.len(), 1);
    assert!(state.announcements.is_empty());
    assert!(state.expire(20, false));
    assert!(state.messages.is_empty());
}
