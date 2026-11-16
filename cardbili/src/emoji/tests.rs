use super::*;

#[test]
fn plain_characters_and_isolated_selectors_are_handled() {
    let chars: String = tokens("中文ab\u{fe0e}\u{fe0f}\u{200d}")
        .map(|token| {
            let Token::Character(character) = token else {
                panic!("expected plain character")
            };
            character
        })
        .collect();
    assert_eq!(chars, "中文ab");
    assert_eq!(tokens("").count(), 0);
}

#[test]
fn text_presentation_selector_keeps_character_glyph() {
    let tokens: Vec<_> = tokens("❤\u{fe0e}").collect();
    assert!(matches!(&tokens[..], [Token::Character('❤')]));
    assert!(matches!(
        &super::tokens("❤️").collect::<Vec<_>>()[..],
        [Token::Emoji(_)]
    ));
}

#[test]
fn combined_emoji_sequences_use_longest_match() {
    for text in ["😀", "👍🏽", "👨‍👩‍👧‍👦", "🇨🇳", "1️⃣"] {
        assert!(
            matches!(&tokens(text).collect::<Vec<_>>()[..], [Token::Emoji(_)]),
            "text={text}"
        );
    }
}

#[test]
fn prepared_text_keeps_utf8_byte_offsets_for_emoji_placeholders() {
    let (text, glyphs) = prepare("中😀文❤️");
    assert_eq!(text, "中\u{fffc}文\u{fffc}");
    assert_eq!(glyphs.len(), 2);
    assert_eq!((glyphs[0].0, glyphs[1].0), (3, 9));
    assert!(glyphs.iter().all(|entry| entry.2 == 12));
}
