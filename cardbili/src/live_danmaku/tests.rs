use super::*;
use std::io::Write;

fn decode_all(bytes: &[u8], budget: usize) -> Result<Vec<(u32, Vec<u8>)>, String> {
    let mut output = Vec::new();
    decode(bytes, 0, &mut { budget }, &mut |operation, body| {
        output.push((operation, body.to_vec()));
        Ok(())
    })?;
    Ok(output)
}

fn compressed(version: u16, body: &[u8]) -> Vec<u8> {
    let payload = if version == 2 {
        let mut writer =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        writer.write_all(body).unwrap();
        writer.finish().unwrap()
    } else {
        let mut output = Vec::new();
        {
            let mut writer = brotli::CompressorWriter::new(&mut output, 4096, 3, 22);
            writer.write_all(body).unwrap();
        }
        output
    };
    let mut bytes = packet(5, &payload);
    bytes[6..8].copy_from_slice(&version.to_be_bytes());
    bytes
}

#[test]
fn packet_header_uses_network_byte_order() {
    let bytes = packet(7, b"{}");
    assert_eq!(
        &bytes[..16],
        &[0, 0, 0, 18, 0, 16, 0, 1, 0, 0, 0, 7, 0, 0, 0, 1]
    );
    assert_eq!(&bytes[16..], b"{}");
}

#[test]
fn decoder_emits_concatenated_packets_in_order() {
    let bytes = [packet(3, &[0, 0, 0, 42]), packet(5, b"message")].concat();
    assert_eq!(
        decode_all(&bytes, MAX_PACKET).unwrap(),
        [(3, vec![0, 0, 0, 42]), (5, b"message".to_vec())]
    );
    assert!(decode_all(&[], MAX_PACKET).unwrap().is_empty());
}

#[test]
fn decoder_accepts_zlib_and_brotli_envelopes() {
    let body = [packet(5, b"first"), packet(5, b"second")].concat();
    for version in [2, 3] {
        assert_eq!(
            decode_all(&compressed(version, &body), body.len()).unwrap(),
            [(5, b"first".to_vec()), (5, b"second".to_vec())]
        );
    }
}

#[test]
fn decoder_rejects_truncated_or_inconsistent_headers() {
    for length in 1..16 {
        assert!(decode_all(&vec![0; length], MAX_PACKET).is_err());
    }
    for (size, header) in [(15u32, 16u16), (100, 16), (16, 15), (16, 17)] {
        let mut bytes = packet(5, &[]);
        bytes[..4].copy_from_slice(&size.to_be_bytes());
        bytes[4..6].copy_from_slice(&header.to_be_bytes());
        assert!(decode_all(&bytes, MAX_PACKET).is_err());
    }
}

#[test]
fn decoder_skips_extended_headers_and_unknown_versions() {
    let mut extended = packet(5, b"xxbody");
    extended[4..6].copy_from_slice(&18u16.to_be_bytes());
    assert_eq!(
        decode_all(&extended, MAX_PACKET).unwrap(),
        [(5, b"body".to_vec())]
    );
    let mut unknown = packet(5, b"ignored");
    unknown[6..8].copy_from_slice(&99u16.to_be_bytes());
    unknown.extend(packet(3, b"ok"));
    assert_eq!(
        decode_all(&unknown, MAX_PACKET).unwrap(),
        [(3, b"ok".to_vec())]
    );
}

#[test]
fn decompression_budget_is_shared_across_packets() {
    let body = packet(5, b"message");
    for version in [2, 3] {
        let bytes = compressed(version, &body);
        assert!(decode_all(&bytes, body.len() - 1).is_err());
        assert!(decode_all(&[bytes.clone(), bytes].concat(), body.len()).is_err());
    }
}

#[test]
fn decoder_limits_nesting_and_total_input_size() {
    let body = packet(5, b"message");
    assert!(decode_all(&compressed(2, &compressed(3, &body)), MAX_PACKET).is_ok());
    assert!(decode_all(
        &compressed(2, &compressed(3, &compressed(2, &body))),
        MAX_PACKET
    )
    .is_err());
    assert!(decode_all(&vec![0; MAX_PACKET + 1], MAX_PACKET).is_err());
}

#[test]
fn decoder_propagates_consumer_errors() {
    let mut budget = MAX_PACKET;
    let error = decode(&packet(5, b"x"), 0, &mut budget, &mut |_, _| {
        Err("consumer failed".into())
    })
    .unwrap_err();
    assert_eq!(error, "consumer failed");
}

#[test]
fn danmaku_parser_handles_command_suffix_color_mask_and_unicode_limit() {
    let text = "中😀".repeat(80);
    let dm = parse_danmaku(
        &json!({"cmd": "DANMU_MSG:4:0:2:2:2:0", "info": [[0, 5, 25, 0xff123456u32], text]}),
    )
    .unwrap();
    assert_eq!((dm.mode, dm.color), (5, 0x123456));
    assert_eq!(dm.text, "中😀".repeat(60));
    let defaults = parse_danmaku(&json!({"cmd": "DANMU_MSG", "info": [[], "hi"]})).unwrap();
    assert_eq!((defaults.mode, defaults.color), (1, 0xffffff));
}

#[test]
fn message_parsers_ignore_unrelated_and_empty_payloads() {
    for value in [
        json!({}),
        json!({"cmd": "OTHER", "info": [[], "hi"]}),
        json!({"cmd": "DANMU_MSG", "info": [[], " \n"]}),
    ] {
        assert!(parse_danmaku(&value).is_none());
    }
    assert_eq!(
        parse_audience(
            &json!({"cmd": "ONLINE_RANK_COUNT", "data": {"online_count_text": "1.2万"}})
        )
        .as_deref(),
        Some("1.2万")
    );
    assert!(parse_audience(
        &json!({"cmd": "ONLINE_RANK_COUNT", "data": {"online_count_text": ""}})
    )
    .is_none());
    assert!(parse_audience(&json!({"cmd": "OTHER", "data": {"online_count_text": "1"}})).is_none());
}
