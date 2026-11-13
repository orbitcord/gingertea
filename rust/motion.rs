//! Small, clock-driven terminal effects. No timers or workers survive their events.
use ratatui::style::Color;
use std::time::{Duration, Instant};
pub const RESOLVE_MS: u64 = 700;
const ACK_MS: u64 = 220;
const READY_MS: u64 = 240;
const FINISH_MS: u64 = 280;
const WORD: &str = "gingertea";

pub struct Motion {
    pub enabled: bool,
    launched: Instant,
    pub pasted: Option<Instant>,
    pub ready: Option<Instant>,
    pub finished: Option<(Instant, bool)>,
}
impl Motion {
    pub fn new(enabled: bool, now: Instant) -> Self {
        Self {
            enabled,
            launched: now,
            pasted: None,
            ready: None,
            finished: None,
        }
    }
    pub fn elapsed(&self, now: Instant) -> u64 {
        now.saturating_duration_since(self.launched).as_millis() as u64
    }
    pub fn interval(&self, now: Instant, busy: bool) -> Option<Duration> {
        if !self.enabled {
            return None;
        }
        let transient = self.elapsed(now) < RESOLVE_MS
            || strength(self.pasted, now, ACK_MS) > 0.
            || strength(self.ready, now, READY_MS) > 0.
            || self
                .finished
                .is_some_and(|(at, _)| strength(Some(at), now, FINISH_MS) > 0.);
        if transient {
            Some(Duration::from_millis(50))
        } else if busy {
            Some(Duration::from_millis(100))
        } else {
            None
        }
    }
    pub fn logo(&self, now: Instant) -> Vec<(char, bool)> {
        if self.enabled {
            resolve(self.elapsed(now))
        } else {
            WORD.chars().map(|c| (c, false)).collect()
        }
    }
    pub fn input_strength(&self, now: Instant) -> f64 {
        if self.enabled {
            strength(self.pasted, now, ACK_MS)
        } else {
            0.
        }
    }
    pub fn ready_strength(&self, now: Instant) -> f64 {
        if self.enabled {
            strength(self.ready, now, READY_MS)
        } else {
            0.
        }
    }
    pub fn finished_strength(&self, now: Instant) -> f64 {
        if self.enabled {
            strength(self.finished.map(|(at, _)| at), now, FINISH_MS)
        } else {
            0.
        }
    }
    pub fn spinner(&self, now: Instant) -> &'static str {
        if self.enabled {
            spinner(self.elapsed(now))
        } else {
            "▶"
        }
    }
    pub fn phase(&self, now: Instant) -> usize {
        (self.elapsed(now) / 100) as usize
    }
}
fn strength(at: Option<Instant>, now: Instant, duration: u64) -> f64 {
    at.map(|at| {
        (1. - now.saturating_duration_since(at).as_millis() as f64 / duration as f64)
            .clamp(0., 1.)
            .powi(3)
    })
    .unwrap_or(0.)
}
pub fn resolve(ms: u64) -> Vec<(char, bool)> {
    let glyphs = ['/', ':', '+', '*', '#', '~'];
    WORD.chars()
        .enumerate()
        .map(|(i, c)| {
            let progress = ms as f64 / RESOLVE_MS as f64 * 11. - i as f64;
            if ms >= RESOLVE_MS || progress >= 1. {
                (c, false)
            } else if progress < 0. {
                ('·', false)
            } else {
                (glyphs[(ms as usize / 65 + i * 3) % glyphs.len()], true)
            }
        })
        .collect()
}
pub fn spinner(ms: u64) -> &'static str {
    ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"][(ms as usize / 100) % 10]
}
pub fn mix(from: Color, to: Color, amount: f64) -> Color {
    if let (Color::Rgb(r, g, b), Color::Rgb(x, y, z)) = (from, to) {
        let blend =
            |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * amount.clamp(0., 1.)).round() as u8;
        Color::Rgb(blend(r, x), blend(g, y), blend(b, z))
    } else {
        from
    }
}
pub fn percentage(status: &str) -> Option<f64> {
    let end = status.find('%')?;
    let number = status[..end]
        .rsplit(|c: char| !c.is_ascii_digit() && c != '.')
        .next()?;
    let value = number.parse::<f64>().ok()?;
    value.is_finite().then(|| (value / 100.).clamp(0., 1.))
}
pub fn progress_bar(fraction: Option<f64>, phase: usize, animated: bool) -> String {
    const WIDTH: usize = 10;
    let mut out = String::new();
    if let Some(value) = fraction {
        let eighths = (value.clamp(0., 1.) * WIDTH as f64 * 8.).round() as usize;
        let partial = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉'];
        for i in 0..WIDTH {
            let n = eighths.saturating_sub(i * 8).min(8);
            out.push(if n == 8 {
                '█'
            } else if n == 0 {
                '─'
            } else {
                partial[n]
            });
        }
    } else {
        let head = phase % (WIDTH + 3);
        for i in 0..WIDTH {
            out.push(if animated && i <= head && head - i < 3 {
                '━'
            } else {
                '─'
            });
        }
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolve_preserves_width_and_settles() {
        for ms in (0..1000).step_by(25) {
            assert_eq!(resolve(ms).len(), 9);
        }
        assert_eq!(
            resolve(700).iter().map(|(c, _)| c).collect::<String>(),
            WORD
        );
        assert_eq!(resolve(700), resolve(9000));
        assert_ne!(resolve(200), resolve(400));
    }
    #[test]
    fn scheduler_stops_and_disabled_never_ticks() {
        let now = Instant::now();
        let mut m = Motion::new(true, now);
        assert!(m.interval(now, false).is_some());
        assert!(m.interval(now + Duration::from_secs(1), false).is_none());
        m.pasted = Some(now + Duration::from_secs(1));
        assert!(
            m.interval(now + Duration::from_millis(1100), false)
                .is_some()
        );
        assert!(m.interval(now + Duration::from_secs(2), false).is_none());
        assert!(m.interval(now + Duration::from_secs(2), true).is_some());
        m.enabled = false;
        assert!(m.interval(now, true).is_none());
        assert_eq!(m.logo(now).iter().map(|(c, _)| c).collect::<String>(), WORD);
    }
    #[test]
    fn progress_is_truthful_and_fixed_width() {
        assert_eq!(percentage("Converting 30%"), Some(0.3));
        assert_eq!(percentage("  15.3% 1MiB/s ETA 00:03"), Some(0.153));
        assert_eq!(percentage("Merging video"), None);
        assert_eq!(progress_bar(Some(1.), 0, true), "██████████");
        for phase in 0..30 {
            assert_eq!(progress_bar(None, phase, true).chars().count(), 10);
        }
        assert_eq!(progress_bar(None, 1, false), progress_bar(None, 5, false));
    }
    #[test]
    fn acknowledgment_decays_without_reversing() {
        let now = Instant::now();
        let mut m = Motion::new(true, now);
        m.pasted = Some(now);
        assert_eq!(m.input_strength(now), 1.);
        assert!(m.input_strength(now + Duration::from_millis(100)) < 1.);
        assert_eq!(m.input_strength(now + Duration::from_millis(220)), 0.);
    }
}
