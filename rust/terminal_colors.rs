//! Terminal output adaptation, after drawing: Apple Terminal needs indexed colors.
use ratatui::{buffer::Buffer, style::Color};
pub fn truecolor() -> bool {
    let program = std::env::var("TERM_PROGRAM").unwrap_or_default();
    if program == "Apple_Terminal" {
        return false;
    }
    let color = std::env::var("COLORTERM")
        .unwrap_or_default()
        .to_lowercase();
    ["truecolor", "24bit"].contains(&color.as_str())
        || ["iTerm.app", "WezTerm", "ghostty", "vscode"].contains(&program.as_str())
}
pub fn indexed(color: Color) -> Color {
    let Color::Rgb(r, g, b) = color else {
        return color;
    };
    let levels = [0i32, 95, 135, 175, 215, 255];
    let nearest = |v: u8| {
        (0..6)
            .min_by_key(|i| (levels[*i] - v as i32).pow(2))
            .unwrap()
    };
    let ri = nearest(r);
    let gi = nearest(g);
    let bi = nearest(b);
    let cube_distance = (levels[ri] - r as i32).pow(2)
        + (levels[gi] - g as i32).pow(2)
        + (levels[bi] - b as i32).pow(2);
    let gray = (((r as f64 + g as f64 + b as f64) / 3. - 8.) / 10.)
        .round()
        .clamp(0., 23.) as i32;
    let value = 8 + gray * 10;
    let gray_distance =
        (value - r as i32).pow(2) + (value - g as i32).pow(2) + (value - b as i32).pow(2);
    Color::Indexed(if gray_distance < cube_distance {
        232 + gray as u8
    } else {
        16 + (36 * ri + 6 * gi + bi) as u8
    })
}
pub fn adapt(buffer: &mut Buffer, rgb: bool) {
    if !rgb {
        for cell in &mut buffer.content {
            cell.fg = indexed(cell.fg);
            cell.bg = indexed(cell.bg);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_palette_colors() {
        assert_eq!(indexed(Color::Rgb(255, 135, 0)), Color::Indexed(208));
        assert_eq!(indexed(Color::Rgb(175, 215, 135)), Color::Indexed(150));
        assert_eq!(indexed(Color::Rgb(18, 18, 18)), Color::Indexed(233));
        assert_eq!(indexed(Color::Reset), Color::Reset);
    }
}
