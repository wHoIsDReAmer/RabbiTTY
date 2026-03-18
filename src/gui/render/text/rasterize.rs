use ab_glyph::FontArc;
use std::fs;

use crate::terminal::font::{load_cjk_fallback_font, load_system_font_by_family};

const DEJAVU_SANS_MONO: &[u8] = include_bytes!("../../../../fonts/DejaVuSansMono.ttf");
pub(super) const COPY_BYTES_PER_ROW_ALIGNMENT: u32 = 256;

/// 5-tap LCD filter weights (FreeType-style), sum = 272.
pub(super) const LCD_WEIGHTS: [u32; 5] = [16, 64, 112, 64, 16];
pub(super) const LCD_WEIGHTS_SUM: u32 = 272;

pub(super) fn align_to(value: u32, alignment: u32) -> u32 {
    if alignment == 0 {
        return value;
    }
    value.div_ceil(alignment) * alignment
}

/// Apply FreeType-style 5-tap LCD filter to a grayscale raster buffer.
pub(super) fn apply_lcd_filter(
    raster_buf: &[u8],
    filter_buf: &mut Vec<u8>,
    raster_width: u32,
    raster_height: u32,
) {
    let raster_len = (raster_width * raster_height) as usize;
    filter_buf.clear();
    filter_buf.resize(raster_len, 0);
    let rw = raster_width as i32;
    for row in 0..raster_height {
        let base = (row * raster_width) as i32;
        for x in 0..rw {
            let mut acc: u32 = 0;
            for (k, &w) in LCD_WEIGHTS.iter().enumerate() {
                let sx = x + k as i32 - 2;
                if sx >= 0 && sx < rw {
                    acc += raster_buf[(base + sx) as usize] as u32 * w;
                }
            }
            filter_buf[(base + x) as usize] = (acc / LCD_WEIGHTS_SUM).min(255) as u8;
        }
    }
}

/// Pack filtered subpixel samples (3 per display pixel) into RGBA with row alignment.
pub(super) fn pack_subpixel_rgba(
    filter_buf: &[u8],
    raster_width: u32,
    raster_height: u32,
    display_width: u32,
) -> Vec<u8> {
    let rgba_row_bytes = display_width * 4;
    let padded_bytes_per_row = align_to(rgba_row_bytes, COPY_BYTES_PER_ROW_ALIGNMENT);
    let mut padded = vec![0u8; (padded_bytes_per_row * raster_height) as usize];

    for row in 0..raster_height {
        let row_start = (row * raster_width) as usize;
        let row_end = row_start + raster_width as usize;
        for col in 0..display_width {
            let rx = row_start + (col * 3) as usize;
            let r = if rx < row_end { filter_buf[rx] } else { 0 };
            let g = if rx + 1 < row_end {
                filter_buf[rx + 1]
            } else {
                0
            };
            let b = if rx + 2 < row_end {
                filter_buf[rx + 2]
            } else {
                0
            };
            let idx = (row * padded_bytes_per_row + col * 4) as usize;
            padded[idx] = r;
            padded[idx + 1] = g;
            padded[idx + 2] = b;
            padded[idx + 3] = r.max(g).max(b);
        }
    }
    padded
}

pub(super) fn default_terminal_font() -> FontArc {
    FontArc::try_from_slice(DEJAVU_SANS_MONO).expect("font load failed")
}

pub(super) fn load_cjk_fallback() -> Option<FontArc> {
    load_cjk_fallback_font()
}

fn load_font_from_path(path: &str) -> Option<FontArc> {
    let bytes = fs::read(path).ok()?;
    FontArc::try_from_vec(bytes).ok()
}

pub(super) fn load_font_from_selection(selection: &str) -> Option<FontArc> {
    load_system_font_by_family(selection).or_else(|| load_font_from_path(selection))
}
