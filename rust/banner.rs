//! The original terminal-cell branding and tea palette, ported from logo.py/theme.py.
use crate::motion;
use ratatui::{prelude::*, widgets::Paragraph};
use serde_json::Value;
use std::sync::OnceLock;
fn branding() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(include_str!("branding.json")).unwrap())
}
#[derive(Clone, Copy)]
pub struct TeaColors {
    pub background: Color,
    pub foreground: Color,
    pub primary: Color,
    pub secondary: Color,
    pub panel: Color,
    pub muted: Color,
    pub success: Color,
    pub error: Color,
}
pub fn colors(name: &str) -> TeaColors {
    let themes = &branding()["themes"];
    let theme = if themes[name].is_object() {
        &themes[name]
    } else {
        &themes["masala-chai"]
    };
    let get = |key: &str| {
        let hex = theme[key].as_str().unwrap_or("#121212");
        Color::Rgb(
            u8::from_str_radix(&hex[1..3], 16).unwrap(),
            u8::from_str_radix(&hex[3..5], 16).unwrap(),
            u8::from_str_radix(&hex[5..7], 16).unwrap(),
        )
    };
    let background = get("background");
    let foreground = get("foreground");
    TeaColors {
        background,
        foreground,
        primary: get("primary"),
        secondary: get("secondary"),
        panel: get("panel"),
        muted: motion::mix(background, foreground, 0.60),
        success: get("success"),
        error: get("error"),
    }
}
fn luminance(c: Color) -> f64 {
    if let Color::Rgb(r, g, b) = c {
        (0.2126 * r as f64 + 0.7152 * g as f64 + 0.0722 * b as f64) / 255.
    } else {
        0.
    }
}
fn legible(c: Color, bg: Color) -> Color {
    let dark = luminance(bg) < 0.5;
    let toward = if dark {
        Color::Rgb(255, 255, 255)
    } else {
        Color::Rgb(0, 0, 0)
    };
    for step in 0..9 {
        let value = motion::mix(c, toward, step as f64 / 10.);
        let lum = luminance(value);
        if dark && lum >= 0.45 || !dark && lum <= 0.45 {
            return value;
        }
    }
    motion::mix(c, toward, 0.8)
}
pub fn shape(width: u16) -> Vec<String> {
    let key = if width >= 72 {
        "shadow"
    } else if width >= 54 {
        "big"
    } else {
        return vec![if width < 9 {
            "GINGERTEA".chars().take(width as usize).collect()
        } else {
            let title = "g i n g e r t e a";
            let side = (width as usize).saturating_sub(title.len() + 4) / 2;
            let side = side.min(14);
            if side > 0 {
                format!("{}  {title}  {}", "━".repeat(side), "━".repeat(side))
            } else {
                title.into()
            }
        }];
    };
    branding()[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect()
}
pub fn lines(width: u16, theme: TeaColors, elapsed: u64, animated: bool) -> Vec<Line<'static>> {
    let raw = shape(width);
    let primary = legible(theme.primary, theme.background);
    let secondary = legible(theme.secondary, theme.background);
    let dark = luminance(theme.background) < 0.5;
    let rows = raw.len();
    raw.iter()
        .enumerate()
        .map(|(row, line)| {
            let total = line.chars().count().max(2) - 1;
            let spans = line
                .chars()
                .enumerate()
                .map(|(col, c)| {
                    let base = if rows == 1 && line.contains('━') {
                        let side = ((width as usize).saturating_sub(21) / 2).min(14);
                        let muted = motion::mix(theme.panel, theme.foreground, 0.25);
                        if col < side {
                            motion::mix(
                                muted,
                                primary,
                                col as f64 / side.saturating_sub(1).max(1) as f64,
                            )
                        } else if col >= side + 21 {
                            motion::mix(
                                secondary,
                                muted,
                                (col - side - 21) as f64 / side.saturating_sub(1).max(1) as f64,
                            )
                        } else {
                            motion::mix(
                                primary,
                                secondary,
                                col.saturating_sub(side + 2) as f64 / 16.,
                            )
                        }
                    } else if width < 9 {
                        primary
                    } else {
                        motion::mix(primary, secondary, col as f64 / total as f64)
                    };
                    let color = if c == '█' {
                        let tint = if dark {
                            Color::Rgb(255, 255, 255)
                        } else {
                            Color::Rgb(0, 0, 0)
                        };
                        let amount = if dark {
                            0.22 * (1. - row as f64 / (rows.saturating_sub(2).max(1)) as f64)
                                .max(0.)
                        } else {
                            0.2 * row as f64 / rows.saturating_sub(1).max(1) as f64
                        };
                        motion::mix(base, tint, amount)
                    } else if rows == 1 {
                        base
                    } else {
                        motion::mix(base, theme.background, 0.6)
                    };
                    let progress = elapsed as f64 / 700. * (total + 3) as f64 - col as f64;
                    let shown = if animated && elapsed < 700 && c != ' ' && progress < 1. {
                        if progress < 0. {
                            '·'
                        } else {
                            ['/', ':', '+', '*', '#', '~']
                                [(elapsed as usize / 65 + col * 3 + row) % 6]
                        }
                    } else {
                        c
                    };
                    Span::styled(
                        shown.to_string(),
                        Style::default().fg(color).add_modifier(
                            if c == '█' || rows == 1 && c != '━' {
                                Modifier::BOLD
                            } else {
                                Modifier::empty()
                            },
                        ),
                    )
                })
                .collect::<Vec<_>>();
            Line::from(spans).alignment(Alignment::Center)
        })
        .collect()
}
pub fn draw(f: &mut Frame, area: Rect, theme: TeaColors, elapsed: u64, animated: bool) {
    f.render_widget(
        Paragraph::new(lines(area.width, theme, elapsed, animated)),
        area,
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_shapes() {
        assert_eq!(shape(116).len(), 6);
        assert_eq!(shape(116)[0].chars().count(), 72);
        assert_eq!(shape(60).len(), 6);
        assert_eq!(shape(40), vec!["━━━━━━━━━  g i n g e r t e a  ━━━━━━━━━"]);
        assert_eq!(shape(5)[0], "GINGE");
    }
    #[test]
    fn settled_banner_matches_original_cells() {
        let t = colors("masala-chai");
        for width in [5, 40, 60, 116] {
            let plain = shape(width);
            let actual = lines(width, t, 700, true);
            for (a, b) in actual.iter().zip(plain) {
                let s = a
                    .spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>();
                assert_eq!(s, b)
            }
        }
    }
    #[test]
    fn themes_keep_their_own_colors() {
        assert_eq!(colors("gingertea-light").primary, Color::Rgb(215, 95, 0));
        assert_ne!(
            colors("gingertea-light").primary,
            colors("tea-house").primary
        );
    }
}
