use crate::{AppWindow, EmojiFont, EmojiPlacement};
use i_slint_core::textlayout::{FontMetrics, TextLayout, TextParagraphLayout, TextShaper};
use slint::{ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::ops::ControlFlow;

pub(crate) const SIZE: usize = 12;
pub(crate) const SPACING: i16 = 1;
pub(crate) const ADVANCE: u16 = SIZE as u16 + 2 * SPACING as u16;
pub(crate) const PLACEHOLDER: char = '\u{fffc}';
static COLOR: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/emoji_color.bin"));
static MONO: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/emoji_mono.bin"));
include!(concat!(env!("OUT_DIR"), "/emoji_font.rs"));

#[derive(Clone, Copy)]
pub(crate) enum Token {
    Character(char),
    Emoji(u16),
}

// 外部表情图使用 65536 起的字形编号
const EXTERNAL_BASE: i32 = 0x1_0000;

#[derive(Default)]
struct Emotes {
    // 每项保存图像和展示边长
    images: Vec<EmoteImage>,
    index: HashMap<String, usize>,
}

struct EmoteImage {
    image: Image,
    size: u32,
    layout_revision: i32,
}

impl Emotes {
    fn find(&self, chunk: &str) -> Option<(usize, usize)> {
        let start = chunk.rfind('[')?;
        self.index.get(&chunk[start..]).map(|&index| (start, index))
    }
}

thread_local! {
    static EMOTES: RefCell<Emotes> = RefCell::new(Emotes::default());
}

pub(crate) fn has_emote(name: &str) -> bool {
    EMOTES.with(|emotes| emotes.borrow().index.contains_key(name))
}

pub(crate) fn layout_changed(text: &str, revision: i32) -> bool {
    EMOTES.with(|emotes| {
        let emotes = emotes.borrow();
        text.split_inclusive(']').any(|chunk| {
            emotes
                .find(chunk)
                .is_some_and(|(_, index)| emotes.images[index].layout_revision > revision)
        })
    })
}

pub(crate) fn register_emotes(ui: &AppWindow, images: Vec<(String, u32, crate::cache::Pixels)>) {
    let global = ui.global::<EmojiFont>();
    let revision = global.get_layout_revision() + 1;
    let mut layout_changed = false;
    EMOTES.with(|emotes| {
        let mut emotes = emotes.borrow_mut();
        for (name, size, (w, h, raw)) in images {
            let image = Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                &raw, w, h,
            ));
            if let Some(&index) = emotes.index.get(&name) {
                let entry = &mut emotes.images[index];
                entry.image = image;
                if entry.size != size {
                    entry.size = size;
                    entry.layout_revision = revision;
                    layout_changed = true;
                }
            } else {
                let index = emotes.images.len();
                emotes.images.push(EmoteImage {
                    image,
                    size,
                    layout_revision: revision,
                });
                emotes.index.insert(name, index);
                layout_changed = true;
            }
        }
    });
    if layout_changed {
        global.set_layout_revision(revision);
    }
    global.set_revision(global.get_revision() + 1);
}

// 最长匹配完整表情，VS16 使用表情字形，VS15 保留文字字形
pub(crate) fn tokens(mut text: &str) -> impl Iterator<Item = Token> + '_ {
    std::iter::from_fn(move || loop {
        let first = text.chars().next()?;
        if matches!(first, '\u{fe0e}' | '\u{fe0f}' | '\u{200d}') {
            text = &text[first.len_utf8()..];
            continue;
        }
        let mut matched = None;
        if text[first.len_utf8()..].starts_with('\u{fe0e}') {
            text = &text[first.len_utf8() + '\u{fe0e}'.len_utf8()..];
            return Some(Token::Character(first));
        }
        let first_text = &text[..first.len_utf8()];
        let index = SEQUENCES.partition_point(|entry| entry.0 < first_text);
        if !SEQUENCES
            .get(index)
            .is_some_and(|entry| entry.0.starts_with(first))
        {
            text = &text[first.len_utf8()..];
            return Some(Token::Character(first));
        }
        let mut prefix = String::new();
        for (offset, character) in text.char_indices() {
            if character == '\u{fe0f}' {
                if let Some((glyph, bytes)) = matched {
                    if bytes == offset {
                        matched = Some((glyph, offset + character.len_utf8()));
                    }
                }
                continue;
            }
            prefix.push(character);
            match SEQUENCES.binary_search_by(|entry| entry.0.cmp(prefix.as_str())) {
                Ok(index) => matched = Some((SEQUENCES[index].1, offset + character.len_utf8())),
                Err(index) => {
                    if !SEQUENCES
                        .get(index)
                        .is_some_and(|entry| entry.0.starts_with(&prefix))
                    {
                        break;
                    }
                }
            }
        }
        if let Some((glyph, bytes)) = matched {
            text = &text[bytes..];
            return Some(Token::Emoji(glyph));
        }
        text = &text[first.len_utf8()..];
        return Some(Token::Character(first));
    })
}

fn prepare(text: &str) -> (String, Vec<(usize, i32, i32)>) {
    let mut prepared = String::with_capacity(text.len());
    let mut emoji = Vec::new();
    EMOTES.with(|emotes| {
        let emotes = emotes.borrow();
        for chunk in text.split_inclusive(']') {
            let matched = emotes.find(chunk);
            let end = matched.map_or(chunk.len(), |(start, _)| start);
            for token in tokens(&chunk[..end]) {
                match token {
                    Token::Character(character) => prepared.push(character),
                    Token::Emoji(glyph) => {
                        emoji.push((prepared.len(), i32::from(glyph), SIZE as i32));
                        prepared.push(PLACEHOLDER);
                    }
                }
            }
            if let Some((_, index)) = matched {
                let size = emotes.images[index].size as i32;
                emoji.push((prepared.len(), EXTERNAL_BASE + index as i32, size));
                prepared.push(PLACEHOLDER);
            }
        }
    });
    (prepared, emoji)
}

fn rich_line(
    text: &str,
    emoji: &[(usize, i32, i32)],
    start: usize,
    end: usize,
    height: i16,
) -> crate::RichLine {
    let mut parts = Vec::new();
    let mut cursor = start;
    let first = emoji.partition_point(|entry| entry.0 < start);
    for &(offset, glyph, size) in emoji[first..].iter().take_while(|entry| entry.0 < end) {
        if cursor < offset {
            parts.push(crate::RichPart {
                text: text[cursor..offset].into(),
                glyph: -1,
                size: 0.0,
            });
        }
        parts.push(crate::RichPart {
            text: Default::default(),
            glyph,
            size: size as f32,
        });
        cursor = offset + PLACEHOLDER.len_utf8();
    }
    if cursor < end {
        parts.push(crate::RichPart {
            text: text[cursor..end].into(),
            glyph: -1,
            size: 0.0,
        });
    }
    crate::RichLine {
        height: f32::from(height),
        parts: slint::ModelRc::new(slint::VecModel::from(parts)),
    }
}

// 文字与表情共用解析结果，表情整体换行，行高取文字和图片的最大值
fn layout_rich(text: &str, width: f32, family: &str, size: f32) -> Vec<crate::RichLine> {
    if width <= 0.0 {
        return Vec::new();
    }
    let font = crate::fonts::LayoutFont::new(family, size);
    let (text, emoji) = prepare(text);
    let mut glyphs = Vec::new();
    font.shape_text(&text, &mut glyphs);
    let mut images = emoji.iter().peekable();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut line_width = 0.0;
    let mut line_height = font.height();
    for glyph in glyphs {
        let offset = glyph.text_byte_offset;
        if text[offset..].starts_with('\n') {
            lines.push(rich_line(&text, &emoji, start, offset, line_height));
            start = offset + 1;
            line_width = 0.0;
            line_height = font.height();
            continue;
        }
        let (advance, height) = match images.peek() {
            Some(entry) if entry.0 == offset => {
                let size = images.next().unwrap().2 as i16;
                (size + 2 * SPACING, size)
            }
            _ => (glyph.advance, font.height()),
        };
        if offset > start && line_width + f32::from(advance) > width {
            lines.push(rich_line(&text, &emoji, start, offset, line_height));
            start = offset;
            line_width = 0.0;
            line_height = font.height();
        }
        line_width += f32::from(advance);
        line_height = line_height.max(height);
    }
    lines.push(rich_line(&text, &emoji, start, text.len(), line_height));
    lines
}

pub(crate) fn pixel(
    glyph: u16,
    offset: usize,
    colored: bool,
    foreground: Rgba8Pixel,
) -> Rgba8Pixel {
    let offset = usize::from(glyph) * SIZE * SIZE + offset;
    // 单色字库缺少的组合保留原始彩色字形
    if colored || !MONOCHROME[usize::from(glyph)] {
        let pixel = &COLOR[offset * 4..offset * 4 + 4];
        Rgba8Pixel::new(pixel[0], pixel[1], pixel[2], pixel[3])
    } else {
        Rgba8Pixel::new(foreground.r, foreground.g, foreground.b, MONO[offset])
    }
}

pub(crate) fn install(ui: &AppWindow) {
    let global = ui.global::<EmojiFont>();
    global.on_monochrome(|glyph| MONOCHROME.get(glyph as usize).copied().unwrap_or(false));
    global.on_prepare(|text, _revision| prepare(&text).0.into());
    global.on_rich_layout(|text, width, family, size, _revision| {
        let lines = layout_rich(&text, width, &family, size);
        crate::RichLayout {
            height: lines.iter().map(|line| line.height).sum(),
            lines: slint::ModelRc::new(slint::VecModel::from(lines)),
        }
    });
    global.on_layout(
        |text, _revision, family, size, width, height, horizontal, vertical, wrap, overflow| {
            let (text, emoji) = prepare(&text);
            if emoji.is_empty() || width <= 0.0 || height <= 0.0 {
                return Default::default();
            }
            let font = crate::fonts::LayoutFont::new(family.as_str(), size);
            let placeholder = font.glyph_for_char(PLACEHOLDER).unwrap().glyph_id.unwrap();
            let paragraph = TextParagraphLayout {
                string: &text,
                layout: TextLayout {
                    font: &font,
                    letter_spacing: None,
                },
                max_width: width as i16,
                max_height: height as i16,
                horizontal_alignment: horizontal,
                vertical_alignment: vertical,
                wrap,
                overflow,
                single_line: false,
            };
            let mut placements = Vec::new();
            let _ = paragraph.layout_lines::<()>(
                |glyphs, x, y, _, _| {
                    let y = y + (font.height() - SIZE as i16) / 2;
                    for glyph in glyphs {
                        if glyph.glyph_id != placeholder
                            || y + SIZE as i16 <= 0
                            || y >= height as i16
                        {
                            continue;
                        }
                        if let Ok(index) =
                            emoji.binary_search_by_key(&glyph.text_byte_offset, |entry| entry.0)
                        {
                            placements.push(EmojiPlacement {
                                glyph: emoji[index].1,
                                x: f32::from(x + glyph.x + SPACING),
                                y: f32::from(y),
                            });
                        }
                    }
                    ControlFlow::Continue(())
                },
                None,
            );
            std::rc::Rc::new(slint::VecModel::from(placements)).into()
        },
    );
    // 小图缓存最多 128 个，像素数据约占 72 KiB
    let cache = RefCell::new(VecDeque::<(u16, bool, Image)>::new());
    global.on_glyph(move |glyph, colored, _revision| {
        if glyph >= EXTERNAL_BASE {
            return EMOTES.with(|emotes| {
                emotes
                    .borrow()
                    .images
                    .get((glyph - EXTERNAL_BASE) as usize)
                    .map(|entry| entry.image.clone())
                    .unwrap_or_default()
            });
        }
        let mut cache = cache.borrow_mut();
        if let Some(index) = cache
            .iter()
            .position(|entry| entry.0 == glyph as u16 && entry.1 == colored)
        {
            let entry = cache.remove(index).unwrap();
            let image = entry.2.clone();
            cache.push_back(entry);
            return image;
        }
        let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(SIZE as u32, SIZE as u32);
        for (offset, target) in buffer.make_mut_slice().iter_mut().enumerate() {
            *target = pixel(
                glyph as u16,
                offset,
                colored,
                Rgba8Pixel::new(255, 255, 255, 255),
            );
        }
        let image = Image::from_rgba8(buffer);
        if cache.len() == 128 {
            cache.pop_front();
        }
        cache.push_back((glyph as u16, colored, image.clone()));
        image
    });
}

#[cfg(test)]
mod tests;
