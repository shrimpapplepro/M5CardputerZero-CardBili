use super::*;
use serde_json::json;

#[test]
fn old_settings_keep_new_field_defaults() {
    let settings: Settings =
        serde_json::from_value(json!({"danmaku_enabled": false, "list_view": 2})).unwrap();
    assert!(!settings.danmaku_enabled);
    assert_eq!(settings.list_view, VideoLayout::TwoColumns);
    assert_eq!(settings.danmaku_opacity, 90);
    assert!(settings.record_history);
    assert!(settings.color_emoji);
    assert!(!settings.pixel_optimize);
    assert_eq!(settings.cdn, "backup");
    assert_eq!(settings.scale_algorithm, ScaleAlgorithm::Area);
}

#[test]
fn settings_round_trip_preserves_every_field() {
    let value = json!({
        "list_view": 0, "danmaku_enabled": false, "danmaku_opacity": 40,
        "menu_position": 2, "record_history": false, "debug_log": true,
        "color_emoji": false, "cdn": "ali", "pixel_optimize": true,
        "scale_algorithm": 4
    });
    let settings: Settings = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(settings).unwrap(), value);
}

#[test]
fn settings_ignore_future_fields_and_reject_wrong_types() {
    assert!(serde_json::from_value::<Settings>(json!({"future_setting": true})).is_ok());
    for value in [
        json!({"list_view": "grid"}),
        json!({"danmaku_enabled": 1}),
        json!({"scale_algorithm": null}),
    ] {
        assert!(serde_json::from_value::<Settings>(value).is_err());
    }
}

#[test]
fn layout_and_menu_indices_clamp_and_cycle_in_both_directions() {
    for index in 0..3 {
        let layout = VideoLayout::from_index(index);
        let menu = MenuPosition::from_index(index);
        assert_eq!(layout.index(), index);
        assert_eq!(menu.index(), index);
        for step in [-7, -1, 0, 1, 7] {
            assert_eq!(layout.cycle(step).index(), (index + step).rem_euclid(3));
            assert_eq!(menu.cycle(step).index(), (index + step).rem_euclid(3));
        }
    }
    assert_eq!(VideoLayout::from_index(-1), VideoLayout::List);
    assert_eq!(VideoLayout::from_index(99), VideoLayout::TwoColumns);
    assert_eq!(MenuPosition::from_index(-1), MenuPosition::Top);
    assert_eq!(MenuPosition::from_index(99), MenuPosition::Right);
}

#[test]
fn scale_algorithms_preserve_persisted_indices_and_ffmpeg_flags() {
    let flags = [
        "neighbor",
        "fast_bilinear",
        "bilinear",
        "bicubic",
        "lanczos",
        "area",
        "spline",
    ];
    for (index, flag) in flags.into_iter().enumerate() {
        let algorithm = ScaleAlgorithm::from_index(index as i32);
        assert_eq!(algorithm.ffmpeg_flag(), flag);
        assert_eq!(serde_json::to_value(algorithm).unwrap(), json!(index));
        assert_eq!(
            serde_json::from_value::<ScaleAlgorithm>(json!(index)).unwrap(),
            algorithm
        );
        assert_eq!(algorithm.cycle(7), algorithm);
    }
    assert_eq!(ScaleAlgorithm::Neighbor.cycle(-1), ScaleAlgorithm::Spline);
    assert_eq!(ScaleAlgorithm::Spline.cycle(1), ScaleAlgorithm::Neighbor);
    assert_eq!(ScaleAlgorithm::from_index(-1), ScaleAlgorithm::Neighbor);
    assert_eq!(ScaleAlgorithm::from_index(99), ScaleAlgorithm::Spline);
}
