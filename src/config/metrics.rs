use ab_glyph::{Font, FontArc, PxScale, ScaleFont, point};

use super::defaults::{DEFAULT_TERMINAL_FONT_SIZE, FONT_SCALE_FACTOR};

const DEJAVU_SANS_MONO: &[u8] = include_bytes!("../../fonts/DejaVuSansMono.ttf");

pub fn cell_metrics_for_font_size(font_size: f32) -> (f32, f32) {
    let font = FontArc::try_from_slice(DEJAVU_SANS_MONO).expect("font load failed");
    cell_metrics_for_font_arc(&font, font_size)
}

/// Calculate cell metrics using a specific font selection.
/// If `font_selection` is empty or the font can't be loaded, falls back to bundled font.
pub fn cell_metrics_for_selection(font_selection: Option<&str>, font_size: f32) -> (f32, f32) {
    let font = font_selection
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(crate::terminal::font::load_system_font_by_family);

    match font {
        Some(ref f) => cell_metrics_for_font_arc(f, font_size),
        None => cell_metrics_for_font_size(font_size),
    }
}

fn cell_metrics_for_font_arc(font: &FontArc, font_size: f32) -> (f32, f32) {
    let scale = PxScale::from(font_size);
    let scaled = font.as_scaled(scale);
    let ascent = scaled.ascent();

    let mut min_y = 0.0;
    let mut max_y = 0.0;
    let mut has_bounds = false;
    for code in 32u8..=126u8 {
        let ch = code as char;
        let glyph_id = font.glyph_id(ch);
        let glyph = glyph_id.with_scale_and_position(scale, point(0.0, ascent));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            if !has_bounds {
                min_y = bounds.min.y;
                max_y = bounds.max.y;
                has_bounds = true;
            } else {
                min_y = min_y.min(bounds.min.y);
                max_y = max_y.max(bounds.max.y);
            }
        }
    }
    let line_height = if has_bounds {
        (max_y - min_y).max(1.0)
    } else {
        scaled.height().max(1.0)
    };

    // For proportional fonts, use max advance across ASCII printable range
    // For monospaced fonts, all advances are the same
    let mut max_advance: f32 = 0.0;
    for code in 32u8..=126u8 {
        let candidate = scaled.h_advance(font.glyph_id(code as char));
        if candidate > max_advance {
            max_advance = candidate;
        }
    }
    if max_advance <= 0.0 {
        max_advance = (line_height * 0.6).max(1.0);
    }

    let cell_height = (font_size / FONT_SCALE_FACTOR).max(1.0);
    let cell_width = max_advance.max(1.0);
    (cell_width, cell_height)
}

pub(super) fn default_cell_metrics() -> (f32, f32) {
    cell_metrics_for_font_size(DEFAULT_TERMINAL_FONT_SIZE)
}
