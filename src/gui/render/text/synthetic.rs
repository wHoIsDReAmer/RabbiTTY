//! Box drawing, block elements and braille drawn to the cell rather than taken
//! from the font. Font glyphs land on fractional positions with antialiased
//! edges, so neighbouring cells never meet: lines break at their joints and
//! block art shows seams where two colours touch.

/// Coverage for one cell, row major, one byte per pixel.
pub(super) struct Coverage {
    pub width: u32,
    pub height: u32,
    pub alpha: Vec<u8>,
}

pub(super) fn is_synthetic(ch: char) -> bool {
    matches!(ch as u32, 0x2500..=0x259F | 0x2800..=0x28FF)
}

pub(super) fn draw(ch: char, width: u32, height: u32) -> Option<Coverage> {
    if width == 0 || height == 0 {
        return None;
    }
    let mut canvas = Canvas::new(width, height);

    match ch as u32 {
        0x2580..=0x259F => blocks(&mut canvas, ch)?,
        0x2800..=0x28FF => braille(&mut canvas, ch),
        0x2500..=0x257F => box_drawing(&mut canvas, ch)?,
        _ => return None,
    }

    Some(Coverage {
        width,
        height,
        alpha: canvas.alpha,
    })
}

struct Canvas {
    width: u32,
    height: u32,
    alpha: Vec<u8>,
}

impl Canvas {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            alpha: vec![0u8; (width * height) as usize],
        }
    }

    /// Half open in both axes, clamped, so adjacent shapes abut without overlap.
    fn fill(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, value: u8) {
        let x0 = x0.round().clamp(0.0, self.width as f32) as u32;
        let x1 = x1.round().clamp(0.0, self.width as f32) as u32;
        let y0 = y0.round().clamp(0.0, self.height as f32) as u32;
        let y1 = y1.round().clamp(0.0, self.height as f32) as u32;

        for y in y0..y1 {
            for x in x0..x1 {
                self.alpha[(y * self.width + x) as usize] = value;
            }
        }
    }

    fn shade(&mut self, numerator: u32, denominator: u32) {
        let value = (255 * numerator / denominator) as u8;
        let (w, h) = (self.width, self.height);
        self.fill(0.0, 0.0, w as f32, h as f32, value);
    }
}

fn blocks(canvas: &mut Canvas, ch: char) -> Option<()> {
    let w = canvas.width as f32;
    let h = canvas.height as f32;

    match ch {
        // Eighths growing from the bottom, then the full block.
        '\u{2581}'..='\u{2588}' => {
            let eighths = (ch as u32 - 0x2580) as f32;
            canvas.fill(0.0, h - h * eighths / 8.0, w, h, 255);
        }
        '\u{2580}' => canvas.fill(0.0, 0.0, w, h / 2.0, 255),
        // Eighths growing from the left.
        '\u{2589}'..='\u{258F}' => {
            let eighths = (0x2590 - ch as u32) as f32;
            canvas.fill(0.0, 0.0, w * eighths / 8.0, h, 255);
        }
        '\u{2590}' => canvas.fill(w / 2.0, 0.0, w, h, 255),
        '\u{2591}' => canvas.shade(1, 4),
        '\u{2592}' => canvas.shade(1, 2),
        '\u{2593}' => canvas.shade(3, 4),
        '\u{2594}' => canvas.fill(0.0, 0.0, w, h / 8.0, 255),
        '\u{2595}' => canvas.fill(w - w / 8.0, 0.0, w, h, 255),
        '\u{2596}' => canvas.fill(0.0, h / 2.0, w / 2.0, h, 255),
        '\u{2597}' => canvas.fill(w / 2.0, h / 2.0, w, h, 255),
        '\u{2598}' => canvas.fill(0.0, 0.0, w / 2.0, h / 2.0, 255),
        '\u{2599}' => {
            canvas.fill(0.0, 0.0, w / 2.0, h, 255);
            canvas.fill(0.0, h / 2.0, w, h, 255);
        }
        '\u{259A}' => {
            canvas.fill(0.0, 0.0, w / 2.0, h / 2.0, 255);
            canvas.fill(w / 2.0, h / 2.0, w, h, 255);
        }
        '\u{259B}' => {
            canvas.fill(0.0, 0.0, w, h / 2.0, 255);
            canvas.fill(0.0, 0.0, w / 2.0, h, 255);
        }
        '\u{259C}' => {
            canvas.fill(0.0, 0.0, w, h / 2.0, 255);
            canvas.fill(w / 2.0, 0.0, w, h, 255);
        }
        '\u{259D}' => canvas.fill(w / 2.0, 0.0, w, h / 2.0, 255),
        '\u{259E}' => {
            canvas.fill(w / 2.0, 0.0, w, h / 2.0, 255);
            canvas.fill(0.0, h / 2.0, w / 2.0, h, 255);
        }
        '\u{259F}' => {
            canvas.fill(w / 2.0, 0.0, w, h, 255);
            canvas.fill(0.0, h / 2.0, w, h, 255);
        }
        _ => return None,
    }
    Some(())
}

/// Dots are numbered down each column: 1-2-3-7 on the left, 4-5-6-8 on the right.
fn braille(canvas: &mut Canvas, ch: char) {
    let pattern = ch as u32 - 0x2800;
    let w = canvas.width as f32;
    let h = canvas.height as f32;
    let dot_w = w / 4.0;
    let dot_h = h / 8.0;

    for bit in 0..8u32 {
        if pattern & (1 << bit) == 0 {
            continue;
        }
        let (col, row) = match bit {
            0 => (0, 0),
            1 => (0, 1),
            2 => (0, 2),
            3 => (1, 0),
            4 => (1, 1),
            5 => (1, 2),
            6 => (0, 3),
            _ => (1, 3),
        };
        let cx = w * (col as f32 * 2.0 + 1.0) / 4.0;
        let cy = h * (row as f32 * 2.0 + 1.0) / 8.0;
        canvas.fill(
            cx - dot_w / 2.0,
            cy - dot_h,
            cx + dot_w / 2.0,
            cy + dot_h,
            255,
        );
    }
}

/// Which arms a box drawing character extends, and how heavy each one is.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
struct Arms {
    up: u8,
    down: u8,
    left: u8,
    right: u8,
    double: bool,
}

fn box_drawing(canvas: &mut Canvas, ch: char) -> Option<()> {
    let arms = arms_for(ch)?;
    let w = canvas.width as f32;
    let h = canvas.height as f32;

    let light = (h / 8.0).max(1.0).round();
    let heavy = (light * 2.0).min(h / 2.0);
    let thickness = |weight: u8| if weight >= 2 { heavy } else { light };

    let mid_x = w / 2.0;
    let mid_y = h / 2.0;

    // Doubles are drawn as two thin rails either side of the centre line.
    let rails = |t: f32| -> Vec<(f32, f32)> {
        if arms.double {
            let gap = t;
            vec![(-gap - t, -gap), (gap, gap + t)]
        } else {
            vec![(-t / 2.0, t / 2.0)]
        }
    };

    if arms.left > 0 {
        let t = thickness(arms.left);
        for (a, b) in rails(t) {
            canvas.fill(0.0, mid_y + a, mid_x + t, mid_y + b, 255);
        }
    }
    if arms.right > 0 {
        let t = thickness(arms.right);
        for (a, b) in rails(t) {
            canvas.fill(mid_x - t, mid_y + a, w, mid_y + b, 255);
        }
    }
    if arms.up > 0 {
        let t = thickness(arms.up);
        for (a, b) in rails(t) {
            canvas.fill(mid_x + a, 0.0, mid_x + b, mid_y + t, 255);
        }
    }
    if arms.down > 0 {
        let t = thickness(arms.down);
        for (a, b) in rails(t) {
            canvas.fill(mid_x + a, mid_y - t, mid_x + b, h, 255);
        }
    }
    Some(())
}

fn arms_for(ch: char) -> Option<Arms> {
    let light = Arms {
        up: 0,
        down: 0,
        left: 0,
        right: 0,
        double: false,
    };
    let a = |up, down, left, right| Arms {
        up,
        down,
        left,
        right,
        ..light
    };
    let d = |up, down, left, right| Arms {
        up,
        down,
        left,
        right,
        double: true,
    };

    Some(match ch {
        '\u{2500}' => a(0, 0, 1, 1),
        '\u{2501}' => a(0, 0, 2, 2),
        '\u{2502}' => a(1, 1, 0, 0),
        '\u{2503}' => a(2, 2, 0, 0),
        '\u{250C}' => a(0, 1, 0, 1),
        '\u{250D}' => a(0, 1, 0, 2),
        '\u{250E}' => a(0, 2, 0, 1),
        '\u{250F}' => a(0, 2, 0, 2),
        '\u{2510}' => a(0, 1, 1, 0),
        '\u{2511}' => a(0, 1, 2, 0),
        '\u{2512}' => a(0, 2, 1, 0),
        '\u{2513}' => a(0, 2, 2, 0),
        '\u{2514}' => a(1, 0, 0, 1),
        '\u{2515}' => a(1, 0, 0, 2),
        '\u{2516}' => a(2, 0, 0, 1),
        '\u{2517}' => a(2, 0, 0, 2),
        '\u{2518}' => a(1, 0, 1, 0),
        '\u{2519}' => a(1, 0, 2, 0),
        '\u{251A}' => a(2, 0, 1, 0),
        '\u{251B}' => a(2, 0, 2, 0),
        '\u{251C}' => a(1, 1, 0, 1),
        '\u{2523}' => a(2, 2, 0, 2),
        '\u{2524}' => a(1, 1, 1, 0),
        '\u{252B}' => a(2, 2, 2, 0),
        '\u{252C}' => a(0, 1, 1, 1),
        '\u{2533}' => a(0, 2, 2, 2),
        '\u{2534}' => a(1, 0, 1, 1),
        '\u{253B}' => a(2, 0, 2, 2),
        '\u{253C}' => a(1, 1, 1, 1),
        '\u{254B}' => a(2, 2, 2, 2),
        '\u{2550}' => d(0, 0, 1, 1),
        '\u{2551}' => d(1, 1, 0, 0),
        '\u{2554}' => d(0, 1, 0, 1),
        '\u{2557}' => d(0, 1, 1, 0),
        '\u{255A}' => d(1, 0, 0, 1),
        '\u{255D}' => d(1, 0, 1, 0),
        '\u{2560}' => d(1, 1, 0, 1),
        '\u{2563}' => d(1, 1, 1, 0),
        '\u{2566}' => d(0, 1, 1, 1),
        '\u{2569}' => d(1, 0, 1, 1),
        '\u{256C}' => d(1, 1, 1, 1),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coverage(ch: char, w: u32, h: u32) -> Vec<u8> {
        draw(ch, w, h).expect("synthesised").alpha
    }

    fn at(alpha: &[u8], w: u32, x: u32, y: u32) -> u8 {
        alpha[(y * w + x) as usize]
    }

    #[test]
    fn the_full_block_covers_every_pixel_of_the_cell() {
        let alpha = coverage('\u{2588}', 9, 20);
        assert!(
            alpha.iter().all(|&v| v == 255),
            "a gap anywhere shows as a seam against the neighbouring cell"
        );
    }

    #[test]
    fn the_upper_half_block_splits_the_cell_exactly() {
        let (w, h) = (8, 20);
        let alpha = coverage('\u{2580}', w, h);

        assert_eq!(at(&alpha, w, 0, 0), 255, "top left is covered");
        assert_eq!(at(&alpha, w, w - 1, h / 2 - 1), 255, "up to the midpoint");
        assert_eq!(at(&alpha, w, 0, h / 2), 0, "and not past it");
        assert_eq!(at(&alpha, w, w - 1, h - 1), 0);
    }

    #[test]
    fn upper_and_lower_halves_tile_without_overlap_or_gap() {
        let (w, h) = (7, 15);
        let upper = coverage('\u{2580}', w, h);
        let lower = coverage('\u{2584}', w, h);

        for i in 0..(w * h) as usize {
            assert_eq!(
                (upper[i] > 0) as u8 + (lower[i] > 0) as u8,
                1,
                "pixel {i} is covered by both halves or by neither, at an odd cell height"
            );
        }
    }

    #[test]
    fn a_horizontal_line_reaches_both_cell_edges() {
        let (w, h) = (9, 20);
        let alpha = coverage('\u{2500}', w, h);

        let row = (0..h).find(|&y| at(&alpha, w, 0, y) > 0).expect("a line");
        assert_eq!(
            at(&alpha, w, w - 1, row),
            255,
            "stopping short of the edge is what breaks the joint with the next cell"
        );
    }

    #[test]
    fn a_vertical_line_reaches_both_cell_edges() {
        let (w, h) = (9, 20);
        let alpha = coverage('\u{2502}', w, h);

        let col = (0..w).find(|&x| at(&alpha, w, x, 0) > 0).expect("a line");
        assert_eq!(at(&alpha, w, col, h - 1), 255, "and the row below joins it");
    }

    #[test]
    fn a_corner_only_extends_the_arms_it_has() {
        let (w, h) = (9, 20);
        let alpha = coverage('\u{250C}', w, h);

        assert_eq!(at(&alpha, w, w - 1, h / 2), 255, "right arm");
        assert_eq!(at(&alpha, w, w / 2, h - 1), 255, "down arm");
        assert_eq!(at(&alpha, w, 0, h / 2), 0, "no left arm");
        assert_eq!(at(&alpha, w, w / 2, 0), 0, "no up arm");
    }

    #[test]
    fn a_heavy_line_is_thicker_than_a_light_one() {
        let (w, h) = (9, 20);
        let light = coverage('\u{2500}', w, h);
        let heavy = coverage('\u{2501}', w, h);

        let count = |a: &[u8]| a.iter().filter(|&&v| v > 0).count();
        assert!(count(&heavy) > count(&light));
    }

    #[test]
    fn a_double_line_leaves_a_gap_between_its_rails() {
        let (w, h) = (9, 24);
        let alpha = coverage('\u{2550}', w, h);

        let column: Vec<u8> = (0..h).map(|y| at(&alpha, w, w / 2, y)).collect();
        let runs = column
            .windows(2)
            .filter(|pair| (pair[0] > 0) != (pair[1] > 0))
            .count();
        assert!(
            runs >= 3,
            "two rails with a gap means at least three transitions, got {runs}"
        );
    }

    #[test]
    fn a_braille_cell_places_only_the_dots_that_are_set() {
        let (w, h) = (8, 16);
        let empty = coverage('\u{2800}', w, h);
        assert!(empty.iter().all(|&v| v == 0), "no dots set");

        let all = coverage('\u{28FF}', w, h);
        assert!(all.iter().any(|&v| v > 0), "eight dots set");

        let one = coverage('\u{2801}', w, h);
        let count = |a: &[u8]| a.iter().filter(|&&v| v > 0).count();
        assert!(count(&one) > 0 && count(&one) < count(&all));
    }

    #[test]
    fn only_the_ranges_we_draw_are_claimed() {
        assert!(is_synthetic('\u{2500}'));
        assert!(is_synthetic('\u{2588}'));
        assert!(is_synthetic('\u{28FF}'));
        assert!(!is_synthetic('a'));
        assert!(!is_synthetic('한'));
        assert!(
            !is_synthetic('\u{25A0}'),
            "filled square is left to the font"
        );
    }

    #[test]
    fn an_unmapped_box_character_falls_back_to_the_font() {
        // Diagonals are not line segments, so they stay with the font rather
        // than being drawn as a wrong-looking cross.
        assert!(draw('\u{2571}', 9, 20).is_none());
    }

    #[test]
    fn a_degenerate_cell_is_refused_rather_than_panicking() {
        assert!(draw('\u{2588}', 0, 20).is_none());
        assert!(draw('\u{2588}', 9, 0).is_none());
    }
}
