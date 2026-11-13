use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
pub fn cursor(value: &str, position: Option<usize>) -> usize {
    position
        .unwrap_or_else(|| value.chars().count())
        .min(value.chars().count())
}
pub fn edit(value: &mut String, position: &mut Option<usize>, key: KeyEvent) -> bool {
    let mut at = cursor(value, *position);
    let len = value.chars().count();
    let byte = |n: usize| {
        value
            .char_indices()
            .nth(n)
            .map(|(i, _)| i)
            .unwrap_or(value.len())
    };
    match key.code {
        KeyCode::Home => at = 0,
        KeyCode::End => at = len,
        KeyCode::Left => at = at.saturating_sub(1),
        KeyCode::Right => at = (at + 1).min(len),
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => at = 0,
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => at = len,
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            value.clear();
            at = 0;
        }
        KeyCode::Backspace if at > 0 => {
            value.replace_range(byte(at - 1)..byte(at), "");
            at -= 1;
        }
        KeyCode::Backspace => (),
        KeyCode::Delete if at < len => {
            value.replace_range(byte(at)..byte(at + 1), "");
        }
        KeyCode::Delete => (),
        KeyCode::Char(c)
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER) =>
        {
            value.insert(byte(at), c);
            at += 1;
        }
        _ => return false,
    }
    *position = Some(at);
    true
}
pub fn visible(value: &str, position: Option<usize>, width: usize) -> (String, u16) {
    let at = cursor(value, position);
    let start = at.saturating_sub(width.saturating_sub(1));
    (
        value.chars().skip(start).take(width).collect(),
        (at - start) as u16,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edit_middle_and_unicode() {
        let mut text = "aéz".to_string();
        let mut p = None;
        assert!(edit(
            &mut text,
            &mut p,
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)
        ));
        edit(
            &mut text,
            &mut p,
            KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE),
        );
        assert_eq!(text, "aé!z");
        edit(
            &mut text,
            &mut p,
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
        );
        assert_eq!(text, "aéz");
        edit(
            &mut text,
            &mut p,
            KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
        );
        edit(
            &mut text,
            &mut p,
            KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE),
        );
        assert_eq!(text, "éz");
    }
}
