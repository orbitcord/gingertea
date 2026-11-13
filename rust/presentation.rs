//! Reproduce the original Textual workspace using Ratatui cells and hit regions.
use super::*;
use ratatui::widgets::{BorderType, Borders};

pub(super) struct Workspace {
    pub logo: Rect,
    pub status: Rect,
    pub tabs: Rect,
    pub input: Rect,
    pub card: Rect,
    pub queue: Rect,
    pub footer: Rect,
}
impl Workspace {
    pub fn new(area: Rect, card_height: u16, footer_height: u16) -> Self {
        let short = area.height < 30;
        let x = area.x + 2.min(area.width);
        let width = area.width.saturating_sub(4);
        let logo_height = if short {
            0
        } else {
            banner::shape(width).len() as u16 + 1
        };
        let status_y = area.y + logo_height + u16::from(!short);
        let tabs_y = status_y + 1 + u16::from(!short);
        let tabs_height = if short { 1 } else { 2 };
        let input_y = tabs_y + tabs_height + 1;
        let card_y = input_y + 4;
        let footer_y = area.bottom().saturating_sub(footer_height);
        let card_height = card_height.min(footer_y.saturating_sub(card_y + 3));
        let queue_y = (card_y + card_height + 1).min(footer_y);
        let clip = |r: Rect| r.intersection(area);
        Self {
            logo: clip(Rect::new(
                x,
                area.y + 1,
                width,
                logo_height.saturating_sub(1),
            )),
            status: clip(Rect::new(x, status_y, width, 1)),
            tabs: clip(Rect::new(x, tabs_y, width, tabs_height)),
            input: clip(Rect::new(x, input_y, width.saturating_sub(4), 3)),
            card: clip(Rect::new(x, card_y, width.saturating_sub(4), card_height)),
            queue: clip(Rect::new(
                x,
                queue_y,
                width.saturating_sub(4),
                footer_y.saturating_sub(queue_y),
            )),
            footer: clip(Rect::new(area.x, footer_y, area.width, footer_height)),
        }
    }
}
fn rule(f: &mut Frame, area: Rect, title: &str, color: Color, p: TeaColors) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    f.render_widget(
        Block::default()
            .borders(Borders::TOP)
            .title(Span::styled(
                format!("─{title}"),
                Style::default().fg(p.primary).bold(),
            ))
            .border_style(Style::default().fg(color)),
        area,
    );
}
fn inset(r: Rect, x: u16, y: u16) -> Rect {
    Rect::new(
        r.x + x.min(r.width),
        r.y + y.min(r.height),
        r.width.saturating_sub(x * 2),
        r.height.saturating_sub(y * 2),
    )
}
fn label(key: &str) -> &str {
    match key {
        "download_dir" => "Save downloads to",
        "cookies_browser" => "Browser cookies for videos that require a login",
        "parallel" => "Downloads at once",
        "filename_template" => "Filename",
        "animations" => "Animations",
        "start" => "Start",
        "end" => "End",
        "subtitles" => "Subtitles",
        "languages" => "Languages",
        "sponsorblock" => "SponsorBlock",
        "chapters" => "Chapters",
        "loudnorm" => "Volume",
        "max_kb" => "Maximum size (KB)",
        "strip_exif" => "Location",
        _ => key,
    }
}
fn display_value(key: &str, value: &str) -> String {
    match (key, value) {
        ("animations" | "chapters" | "loudnorm", "true") => "On".into(),
        ("animations" | "chapters" | "loudnorm", "false") => "Off".into(),
        ("cookies_browser", "") => "Off".into(),
        ("subtitles", "embed") => "Inside the video".into(),
        ("subtitles", "srt") => "Separate .srt".into(),
        ("subtitles" | "sponsorblock", "off") => "Off".into(),
        ("sponsorblock", "cut") => "Cut segments".into(),
        ("sponsorblock", "chapters") => "Mark as chapters".into(),
        ("strip_exif", "true") => "Strip location".into(),
        ("strip_exif", "false") => "Keep".into(),
        _ => value.into(),
    }
}
fn date(raw: &str) -> String {
    if raw.len() == 8 && raw.is_ascii() {
        format!("{}-{}-{}", &raw[..4], &raw[4..6], &raw[6..])
    } else {
        raw.into()
    }
}
fn shorten(path: &Path) -> String {
    if let Some(home) = directories::UserDirs::new()
        && let Ok(rel) = path.strip_prefix(home.home_dir())
    {
        return format!("~/{}", rel.display());
    }
    path.display().to_string()
}
fn field_index(form: &Form, key: &str) -> usize {
    form.rows.iter().position(|(k, _)| k == key).unwrap_or(0)
}
fn field_value<'a>(form: &'a Form, key: &str) -> &'a str {
    form.rows
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .unwrap_or("")
}
fn popup(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(2));
    let height = height.min(area.height.saturating_sub(2));
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}
fn footer_keys(files: bool, typing: bool) -> Vec<(&'static str, &'static str, UiAction)> {
    let mut keys = if files {
        vec![
            ("c", "convert", UiAction::Key('c')),
            ("f", "browse", UiAction::Key('f')),
            ("r", "remove", UiAction::Key('r')),
            ("x", "cancel", UiAction::Key('x')),
        ]
    } else {
        vec![
            ("d", "download", UiAction::Key('d')),
            ("e", "choose", UiAction::Key('e')),
            ("c", "convert", UiAction::Key('c')),
            ("a", "audio", UiAction::Key('a')),
            ("x", "cancel", UiAction::Key('x')),
            ("p", "pause", UiAction::Key('p')),
            ("h", "history", UiAction::Key('h')),
            ("s", "settings", UiAction::Key('s')),
            ("l", "sites", UiAction::Key('l')),
            ("v", "paste", UiAction::Key('v')),
        ]
    };
    keys.push(if typing {
        ("tab", "use keys", UiAction::Key('\t'))
    } else {
        (
            "esc",
            if files { "file bar" } else { "url bar" },
            UiAction::Input,
        )
    });
    keys.extend([
        ("t", "themes", UiAction::Key('t')),
        ("q", "quit", UiAction::Key('q')),
    ]);
    keys
}
fn footer_lines(keys: &[(&str, &str, UiAction)], width: u16) -> Vec<Vec<usize>> {
    let room = width.saturating_sub(4) as usize;
    let mut rows = vec![vec![]];
    let mut used = 0;
    for (i, (key, label, _)) in keys.iter().enumerate() {
        let cells = key.len() + 3 + label.len();
        if used > 0 && used + 3 + cells > room {
            rows.push(Vec::new());
            used = 0;
        }
        used += cells + if used > 0 { 3 } else { 0 };
        rows.last_mut().unwrap().push(i);
    }
    rows
}
impl App {
    fn hit(&mut self, rect: Rect, action: UiAction) {
        if rect.width > 0 && rect.height > 0 {
            self.hitboxes.push((rect, action));
        }
    }
    fn button(
        &mut self,
        f: &mut Frame,
        rect: Rect,
        text: &str,
        primary: bool,
        action: UiAction,
        p: TeaColors,
    ) {
        let hovered = self.hovered.as_ref() == Some(&action);
        let fill = if primary {
            if hovered {
                motion::mix(p.primary, p.foreground, 0.12)
            } else {
                p.primary
            }
        } else if hovered {
            motion::mix(p.panel, p.foreground, 0.12)
        } else {
            p.panel
        };
        let color = if primary {
            p.background
        } else if hovered {
            p.primary
        } else {
            p.foreground
        };
        f.render_widget(Block::default().style(Style::default().bg(fill)), rect);
        if rect.height > 0 {
            f.render_widget(
                Paragraph::new(text.to_owned())
                    .centered()
                    .style(Style::default().fg(color).bg(fill).bold()),
                Rect::new(rect.x, rect.y + rect.height / 2, rect.width, 1),
            );
        }
        self.hit(rect, action);
    }
    fn field(&mut self, f: &mut Frame, rect: Rect, index: usize, form: &Form, p: TeaColors) {
        let (key, value) = &form.rows[index];
        let selected = index == form.selected;
        let fill = if selected {
            motion::mix(p.background, p.primary, 0.25)
        } else {
            p.panel
        };
        let text = display_value(key, value);
        f.render_widget(
            Paragraph::new(if text.is_empty() {
                " optional ".into()
            } else {
                format!(" {text} ")
            })
            .style(Style::default().bg(fill).fg(if selected {
                p.foreground
            } else {
                p.muted
            })),
            rect,
        );
        self.hit(rect, UiAction::Field(index));
    }
    pub(super) fn draw_original(&mut self, f: &mut Frame) {
        let area = f.area();
        let p = banner::colors(&self.settings.theme);
        let now = Instant::now();
        self.hitboxes.clear();
        f.render_widget(
            Block::default().style(Style::default().bg(p.background).fg(p.foreground)),
            area,
        );
        let base = if matches!(self.page, Page::History | Page::Sites) {
            self.return_page
        } else {
            self.page
        };
        let files = base == Page::Files;
        let short = area.height < 30;
        let keys = footer_keys(files, self.editing);
        let key_rows = footer_lines(&keys, area.width);
        let footer_height = (key_rows.len() as u16 + 1).clamp(2, 4);
        let card_height = if files {
            if self.files.is_empty() {
                5
            } else {
                2 + self.files.len().min(if short { 5 } else { 12 }) as u16
                    + 1
                    + if short { 1 } else { 3 }
            }
        } else if self.loading_video.is_some() {
            3
        } else if self.probe_error.is_some() {
            6
        } else if self.info.is_some() {
            if short { 5 } else { 13 }
        } else {
            5
        };
        let workspace = Workspace::new(area, card_height, footer_height);
        if workspace.logo.height > 0 {
            banner::draw(
                f,
                workspace.logo,
                p,
                self.motion.elapsed(now),
                self.motion.enabled,
            );
        }
        let saving = if files {
            "saving next to each original".into()
        } else {
            format!("saving to {}", shorten(&self.settings.download_path()))
        };
        let version = if self.backend_version.is_empty() {
            "…"
        } else {
            &self.backend_version
        };
        let status = format!(
            "gingertea {}  ·  yt-dlp {}  ·  {}",
            env!("CARGO_PKG_VERSION"),
            version,
            saving
        );
        f.render_widget(
            Paragraph::new(status)
                .centered()
                .style(Style::default().fg(p.muted)),
            workspace.status,
        );
        let tab_labels = [
            (" Download ", Page::Download),
            (" Convert files ", Page::Files),
        ];
        let mut x = workspace.tabs.x;
        if workspace.tabs.height > 1 {
            f.render_widget(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::default().fg(p.panel)),
                workspace.tabs,
            );
        }
        for (text, page) in tab_labels {
            let rect = Rect::new(x, workspace.tabs.y, text.len() as u16, 1).intersection(area);
            f.render_widget(
                Paragraph::new(text).style(if page == base {
                    Style::default().bg(p.primary).fg(p.background).bold()
                } else {
                    Style::default().fg(p.muted)
                }),
                rect,
            );
            self.hit(
                rect,
                UiAction::Key(if page == Page::Files { '2' } else { '1' }),
            );
            x = x.saturating_add(text.len() as u16 + 2);
        }
        let border = motion::mix(
            if self.editing { p.primary } else { p.panel },
            motion::mix(p.primary, p.foreground, 0.85),
            self.motion.input_strength(now),
        );
        let input = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(Span::styled(
                if files { "─ files " } else { "─ URL " },
                Style::default()
                    .fg(if self.editing { p.primary } else { p.muted })
                    .bold(),
            ))
            .border_style(Style::default().fg(border));
        f.render_widget(input, workspace.input);
        let inner = Rect::new(
            workspace.input.x + 1,
            workspace.input.y + 1,
            workspace.input.width.saturating_sub(2),
            1,
        )
        .intersection(area);
        if inner.width > 0 {
            let placeholder = if files {
                "Drop files here, or press f to browse"
            } else {
                "Paste a video link"
            };
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(" › ", Style::default().fg(p.primary).bold()),
                    Span::styled(
                        if self.input.is_empty() {
                            placeholder.to_owned()
                        } else {
                            self.input.clone()
                        },
                        Style::default().fg(if self.input.is_empty() {
                            p.muted
                        } else {
                            p.foreground
                        }),
                    ),
                ])),
                inner,
            );
            self.hit(inner, UiAction::Input);
            if self.editing
                && self.form.is_none()
                && !matches!(self.page, Page::History | Page::Sites)
                && !self.choosing
                && self.theme_picker.is_none()
            {
                f.set_cursor_position((
                    inner.x
                        + 3
                        + self
                            .input
                            .chars()
                            .count()
                            .min(inner.width.saturating_sub(4) as usize)
                            as u16,
                    inner.y,
                ));
            }
        }
        let ready = motion::mix(p.panel, p.primary, self.motion.ready_strength(now));
        if files {
            self.draw_files(f, workspace.card, p, short, ready);
        } else {
            self.draw_video(f, workspace.card, p, short, ready, now);
        }
        self.draw_queue(f, workspace.queue, p, files, short, now);
        f.render_widget(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(p.panel)),
            workspace.footer,
        );
        for (row, indices) in key_rows
            .iter()
            .take(workspace.footer.height.saturating_sub(1) as usize)
            .enumerate()
        {
            let width = indices
                .iter()
                .map(|i| keys[*i].0.len() + 3 + keys[*i].1.len())
                .sum::<usize>()
                + indices.len().saturating_sub(1) * 3;
            let mut x = area.x + area.width.saturating_sub(width as u16) / 2;
            let y = workspace.footer.y + 1 + row as u16;
            for i in indices {
                let (key, label, action) = &keys[*i];
                let text = Line::from(vec![
                    Span::styled(
                        format!(" {key} "),
                        Style::default().bg(p.panel).fg(p.foreground).bold(),
                    ),
                    Span::styled(
                        format!(" {label}"),
                        Style::default().fg(if self.hovered.as_ref() == Some(action) {
                            p.primary
                        } else {
                            p.muted
                        }),
                    ),
                ]);
                let width = (key.len() + 3 + label.len()) as u16;
                let rect = Rect::new(x, y, width, 1).intersection(area);
                f.render_widget(Paragraph::new(text), rect);
                self.hit(rect, action.clone());
                x += width + 3;
            }
        }
        if self.toast.is_some()
            && !self.status.starts_with("Paste a link")
            && !self.status.contains("item(s)")
            && workspace.queue.height > 3
        {
            let width = (self.status.chars().count() as u16 + 2).min(area.width.saturating_sub(6));
            let rect = Rect::new(
                area.right().saturating_sub(width + 3),
                workspace.footer.y.saturating_sub(1),
                width,
                1,
            )
            .intersection(area);
            f.render_widget(
                Paragraph::new(format!(" {} ", self.status))
                    .style(Style::default().bg(p.panel).fg(p.muted)),
                rect,
            );
        }
        if matches!(self.page, Page::History | Page::Sites) {
            self.hitboxes.clear();
            self.draw_list_dialog(f, area, p);
        }
        if self.choosing {
            self.hitboxes.clear();
            self.draw_playlist_dialog(f, area, p);
        }
        if let Some(form) = self.form.clone() {
            self.hitboxes.clear();
            if form.purpose == "convert" {
                self.draw_convert(f, area, p, &form)
            } else {
                self.draw_settings(f, area, p, &form)
            }
        }
        if let Some(browser) = &self.browser {
            self.hitboxes.clear();
            let popup = popup(area, 80, 32);
            f.render_widget(Clear, popup);
            f.render_widget(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(p.primary))
                    .style(Style::default().bg(p.background)),
                popup,
            );
            let rect = inset(popup, 3, 2);
            let rows = browser
                .entries
                .iter()
                .map(|path| {
                    ListItem::new(format!(
                        "{} {}",
                        if path.is_dir() { "▸" } else { " " },
                        if browser.dir.parent() == Some(path.as_path()) {
                            "..".into()
                        } else {
                            path.file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned()
                        }
                    ))
                })
                .collect::<Vec<_>>();
            let mut state = ListState::default().with_selected(Some(browser.selected));
            f.render_widget(
                Paragraph::new(format!(
                    "Browse files · {} · Enter Open · Esc Close",
                    browser.dir.display()
                ))
                .style(Style::default().fg(p.foreground).bold()),
                Rect::new(rect.x, rect.y, rect.width, 1),
            );
            f.render_stateful_widget(
                List::new(rows).highlight_style(Style::default().bg(p.primary).fg(p.background)),
                Rect::new(
                    rect.x,
                    rect.y + 2,
                    rect.width,
                    rect.height.saturating_sub(3),
                ),
                &mut state,
            );
            f.render_widget(
                Paragraph::new(if self.folder_pick {
                    "Enter Open · Space use folder · Esc Close"
                } else {
                    "Enter Open · Esc Close"
                })
                .style(Style::default().fg(p.muted)),
                Rect::new(rect.x, rect.bottom().saturating_sub(1), rect.width, 1),
            );
            let offset = state.offset();
            for row in 0..rect
                .height
                .saturating_sub(3)
                .min(browser.entries.len().saturating_sub(offset) as u16)
            {
                self.hitboxes.push((
                    Rect::new(rect.x, rect.y + 2 + row, rect.width, 1),
                    UiAction::Browser(offset + row as usize),
                ));
            }
        }
        if let Some(selected) = self.theme_picker {
            self.hitboxes.clear();
            let popup = popup(area, 52, 18);
            f.render_widget(Clear, popup);
            f.render_widget(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(p.primary))
                    .style(Style::default().bg(p.background)),
                popup,
            );
            let rect = inset(popup, 3, 1);
            f.render_widget(
                Paragraph::new("Themes · live preview").bold(),
                Rect::new(rect.x, rect.y, rect.width, 1),
            );
            let body = Rect::new(
                rect.x,
                rect.y + 2,
                rect.width,
                rect.height.saturating_sub(3),
            );
            let rows = THEMES
                .iter()
                .map(|name| {
                    ListItem::new(format!(
                        "{} {name}",
                        if self.theme_original.as_deref() == Some(*name) {
                            "✓"
                        } else {
                            " "
                        }
                    ))
                })
                .collect::<Vec<_>>();
            let mut state = ListState::default()
                .with_offset(self.theme_scroll)
                .with_selected(Some(selected));
            f.render_stateful_widget(
                List::new(rows)
                    .highlight_symbol("› ")
                    .highlight_style(Style::default().bg(p.primary).fg(p.background).bold()),
                body,
                &mut state,
            );
            let offset = state.offset();
            self.theme_scroll = offset;
            for row in 0..body.height.min(THEMES.len().saturating_sub(offset) as u16) {
                self.hit(
                    Rect::new(body.x, body.y + row, body.width, 1),
                    UiAction::Theme(offset + row as usize),
                );
            }
            f.render_widget(
                Paragraph::new(if self.status.starts_with("Could not save theme:") {
                    self.status.as_str()
                } else {
                    "↑↓ / Tab move · Enter save · Esc cancel"
                })
                .style(Style::default().fg(p.muted)),
                Rect::new(rect.x, rect.bottom().saturating_sub(1), rect.width, 1),
            );
        }
    }

    fn draw_video(
        &mut self,
        f: &mut Frame,
        area: Rect,
        p: TeaColors,
        short: bool,
        border: Color,
        now: Instant,
    ) {
        if area.height == 0 {
            return;
        }
        let playlist = self.info.as_ref().is_some_and(|i| i["entries"].is_array());
        rule(
            f,
            area,
            if playlist { "Playlist" } else { "Video" },
            border,
            p,
        );
        let body = Rect::new(
            area.x,
            area.y + if short { 1 } else { 2 },
            area.width,
            area.height.saturating_sub(if short { 1 } else { 2 }),
        );
        if self.loading_video.is_some() {
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        format!("  {}     ", self.motion.spinner(now)),
                        Style::default().fg(p.primary),
                    ),
                    Span::styled(
                        format!("Looking up {}…", self.input),
                        Style::default().fg(p.muted),
                    ),
                ]))
                .wrap(Wrap { trim: true }),
                body,
            );
            return;
        }
        if let Some(error) = self.probe_error.clone() {
            f.render_widget(
                Paragraph::new(format!("✗ {error}"))
                    .style(Style::default().fg(p.error))
                    .wrap(Wrap { trim: true }),
                body,
            );
            if body.height >= 3 {
                self.button(
                    f,
                    Rect::new(body.x, body.bottom() - 1, 15.min(body.width), 1),
                    "Try again",
                    false,
                    UiAction::Retry,
                    p,
                );
            }
            return;
        }
        let Some(info) = self.info.clone() else {
            f.render_widget(Paragraph::new(vec![Line::from(Span::styled("Paste a video link above.",Style::default().fg(p.muted).bold())),Line::raw(""),Line::from(Span::styled("YouTube, Vimeo, SoundCloud and 1,300+ sites. Press Enter to load a typed link.",Style::default().fg(p.muted)))]).wrap(Wrap{trim:false}),body);
            return;
        };
        let thumb_width = if !short && f.area().width >= 96 {
            37
        } else {
            0
        };
        if thumb_width > 0 {
            if let Some(t) = &self.thumb {
                let lines = (0..18)
                    .step_by(2)
                    .map(|y| {
                        Line::from(
                            (0..34)
                                .map(|x| {
                                    let source = x * 36 / 34;
                                    let top = t[y][source];
                                    let bottom = t[y + 1][source];
                                    Span::styled(
                                        "▀",
                                        Style::default()
                                            .fg(Color::Rgb(top.0, top.1, top.2))
                                            .bg(Color::Rgb(bottom.0, bottom.1, bottom.2)),
                                    )
                                })
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect::<Vec<_>>();
                f.render_widget(
                    Paragraph::new(lines),
                    Rect::new(body.x, body.y, 34, 9.min(body.height)),
                );
            } else {
                f.render_widget(
                    Paragraph::new("▶")
                        .centered()
                        .style(Style::default().fg(p.muted)),
                    Rect::new(
                        body.x,
                        body.y + 3.min(body.height.saturating_sub(1)),
                        34.min(body.width),
                        1,
                    ),
                );
            }
        }
        let details = Rect::new(
            body.x + thumb_width,
            body.y,
            body.width.saturating_sub(thumb_width),
            body.height,
        );
        f.render_widget(
            Paragraph::new(info["title"].as_str().unwrap_or("Untitled"))
                .style(Style::default().fg(p.foreground).bold()),
            Rect::new(details.x, details.y, details.width, 1),
        );
        let channel = info["channel"]
            .as_str()
            .or(info["uploader"].as_str())
            .unwrap_or("—");
        let site = info["extractor_key"]
            .as_str()
            .or(info["extractor"].as_str())
            .unwrap_or("—");
        let mut metadata = if playlist {
            vec![
                ("channel", channel.to_owned()),
                ("videos", self.entries.len().to_string()),
                (
                    "folder",
                    format!("{}/", info["title"].as_str().unwrap_or("playlist")),
                ),
            ]
        } else {
            vec![
                ("channel", channel.to_owned()),
                ("length", duration(media::number(&info["duration"]))),
                (
                    "uploaded",
                    date(info["upload_date"].as_str().unwrap_or("—")),
                ),
            ]
        };
        if !playlist {
            let height = info["formats"]
                .as_array()
                .map(|formats| {
                    formats
                        .iter()
                        .map(|v| {
                            let h = media::number(&v["height"]);
                            let w = media::number(&v["width"]);
                            if w > 0. { h.min(w) } else { h }
                        })
                        .fold(0., f64::max)
                })
                .unwrap_or_else(|| media::number(&info["height"]));
            if height > 0. {
                metadata.push(("quality", format!("up to {height}p")));
            }
        }
        metadata.push(("site", site.to_owned()));
        if short {
            let text = metadata
                .iter()
                .map(|(_, v)| v.as_str())
                .collect::<Vec<_>>()
                .join(" · ");
            f.render_widget(
                Paragraph::new(text).style(Style::default().fg(p.muted)),
                Rect::new(details.x, details.y + 1, details.width, 1),
            );
        } else {
            let rows = metadata
                .iter()
                .map(|(key, value)| {
                    Line::from(vec![
                        Span::styled(
                            format!("{key}{} ", "·".repeat(10 - key.len())),
                            Style::default().fg(p.muted),
                        ),
                        Span::styled(value.clone(), Style::default().fg(p.foreground)),
                    ])
                })
                .collect::<Vec<_>>();
            f.render_widget(
                Paragraph::new(rows),
                Rect::new(
                    details.x,
                    details.y + 2,
                    details.width,
                    details.height.saturating_sub(5),
                ),
            );
        }
        let height = if short { 1 } else { 3 };
        if details.height >= height {
            let y = details.bottom() - height;
            let label = if playlist {
                "[d] Download all"
            } else {
                "[d] Download"
            };
            let width = (label.len() as u16 + 8).min(details.width);
            self.button(
                f,
                Rect::new(details.x, y, width, height),
                label,
                true,
                UiAction::Key('d'),
                p,
            );
            let next_x = details.x + width + 2;
            let second = if playlist {
                "[e] Choose"
            } else {
                "[c] Convert"
            };
            let width = (second.len() as u16 + 8).min(details.right().saturating_sub(next_x));
            self.button(
                f,
                Rect::new(next_x, y, width, height),
                second,
                false,
                UiAction::Key(if playlist { 'e' } else { 'c' }),
                p,
            );
            if f.area().width >= 120 && details.right() > next_x + width + 2 {
                f.render_widget(
                    Paragraph::new("MP4 by default").style(Style::default().fg(p.muted)),
                    Rect::new(
                        next_x + width + 2,
                        y,
                        details.right().saturating_sub(next_x + width + 2),
                        height,
                    ),
                );
            }
        }
    }
    fn draw_files(&mut self, f: &mut Frame, area: Rect, p: TeaColors, short: bool, border: Color) {
        if area.height == 0 {
            return;
        }
        let title = if self.files.is_empty() {
            "files".into()
        } else {
            format!(
                "{} file{}",
                self.files.len(),
                if self.files.len() == 1 { "" } else { "s" }
            )
        };
        rule(f, area, &title, border, p);
        let offset = if short { 1 } else { 2 };
        let body = Rect::new(
            area.x,
            area.y + offset,
            area.width,
            area.height.saturating_sub(offset),
        );
        if self.files.is_empty() {
            f.render_widget(Paragraph::new(vec![Line::from(Span::styled("Add files to convert.",Style::default().bold())),Line::raw(""),Line::raw("Drag files from Finder or Explorer into this window, or press f to browse. You can add video, audio and image files in one batch.")]).style(Style::default().fg(p.muted)).wrap(Wrap{trim:false}),body);
            return;
        }
        let button_height = if short { 1 } else { 3 };
        let rows = body.height.saturating_sub(button_height + 1);
        let mut state = ListState::default().with_selected(Some(self.selected));
        let entries = self
            .files
            .iter()
            .map(|file| {
                let video = file.video.as_ref();
                let dimensions = video
                    .map(|v| {
                        format!(
                            "{}×{}",
                            media::number(&v["width"]),
                            media::number(&v["height"])
                        )
                    })
                    .unwrap_or_default();
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!(
                            "▸ {:<32}  ",
                            file.path.file_name().unwrap_or_default().to_string_lossy()
                        ),
                        Style::default().bold(),
                    ),
                    Span::styled(
                        format!(
                            "{} · {} · {}",
                            file.media,
                            dimensions,
                            duration(file.duration)
                        ),
                        Style::default().fg(p.muted),
                    ),
                ]))
            })
            .collect::<Vec<_>>();
        f.render_stateful_widget(
            List::new(entries).highlight_style(Style::default().bg(p.panel)),
            Rect::new(body.x, body.y, body.width, rows),
            &mut state,
        );
        for i in 0..rows.min(self.files.len() as u16) {
            self.hit(
                Rect::new(body.x, body.y + i, body.width, 1),
                UiAction::Select(i as usize),
            );
        }
        let y = body.bottom().saturating_sub(button_height);
        self.button(
            f,
            Rect::new(body.x, y, 23.min(body.width), button_height),
            if self.files.len() > 1 {
                "[c] Convert all"
            } else {
                "[c] Convert"
            },
            true,
            UiAction::Key('c'),
            p,
        );
        if body.width > 25 {
            self.button(
                f,
                Rect::new(body.x + 25, y, 18.min(body.width - 25), button_height),
                "[r] Remove",
                false,
                UiAction::Key('r'),
                p,
            );
        }
        if f.area().width >= 120 && body.width > 45 {
            f.render_widget(
                Paragraph::new("saves next to each original").style(Style::default().fg(p.muted)),
                Rect::new(body.x + 45, y, body.width - 45, button_height),
            );
        }
    }
    fn draw_queue(
        &mut self,
        f: &mut Frame,
        area: Rect,
        p: TeaColors,
        files: bool,
        short: bool,
        now: Instant,
    ) {
        if area.height == 0 {
            return;
        }
        let color = motion::mix(
            p.panel,
            if self.motion.finished.is_some_and(|(_, ok)| ok) {
                p.success
            } else {
                p.error
            },
            self.motion.finished_strength(now),
        );
        rule(
            f,
            area,
            if files {
                "Conversions"
            } else if self.paused {
                "Downloads · paused"
            } else {
                "Downloads"
            },
            color,
            p,
        );
        let offset = if short { 1 } else { 2 };
        let body = Rect::new(
            area.x,
            area.y + offset,
            area.width,
            area.height.saturating_sub(offset),
        );
        let jobs = self
            .jobs
            .iter()
            .enumerate()
            .filter(|(_, j)| j.local == files)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        if jobs.is_empty() {
            f.render_widget(
                Paragraph::new(if files {
                    "No conversions yet. Add files, then choose Convert."
                } else {
                    "No downloads yet. Paste a link, then choose Download."
                })
                .style(Style::default().fg(p.muted)),
                body,
            );
            return;
        }
        let visible = body.height as usize / 3;
        let start = jobs.len().saturating_sub(visible.max(1));
        for (row, index) in jobs.iter().skip(start).take(visible.max(1)).enumerate() {
            let j = &self.jobs[*index];
            let y = body.y + row as u16 * 3;
            if y >= body.bottom() {
                break;
            }
            let color = if j.done {
                if j.error { p.error } else { p.success }
            } else if j.started {
                p.primary
            } else {
                p.muted
            };
            let icon = if j.done {
                if j.error { "✗" } else { "✓" }
            } else if j.started {
                self.motion.spinner(now)
            } else {
                "·"
            };
            let kind = format!(
                "{} · {}{}",
                j.choice.fmt.to_uppercase(),
                j.choice.quality,
                if j.choice.quality == "best" {
                    ""
                } else if j.choice.kind == "video" {
                    "p"
                } else {
                    "k"
                }
            );
            let controls = (kind.chars().count() + 10) as u16;
            let title_width = body.width.saturating_sub(controls + 2);
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(format!("{icon} "), Style::default().fg(color).bold()),
                    Span::styled(j.title.clone(), Style::default().fg(p.foreground).bold()),
                ])),
                Rect::new(body.x, y, title_width, 1),
            );
            if body.width > controls {
                let x = body.right() - controls;
                f.render_widget(
                    Paragraph::new(kind).style(Style::default().fg(p.primary)),
                    Rect::new(x, y, controls.saturating_sub(8), 1),
                );
                if !j.done {
                    let cancel = Rect::new(body.right().saturating_sub(7), y, 7, 1);
                    f.render_widget(
                        Paragraph::new("cancel").style(Style::default().fg(p.muted)),
                        cancel,
                    );
                    self.hitboxes.push((cancel, UiAction::CancelJob(*index)));
                }
            }
            if y + 1 >= body.bottom() {
                continue;
            }
            let width = 24.min(body.width.saturating_sub(2));
            let fraction = if j.done {
                Some(if j.error { 0. } else { 1. })
            } else if !j.started {
                Some(0.)
            } else {
                j.fraction
            };
            let head = self.motion.phase(now) % (width as usize + 4).max(1);
            let spans = (0..width)
                .map(|i| {
                    let filled = if let Some(fraction) = fraction {
                        (i as f64) < (width as f64 * fraction).round()
                    } else {
                        self.motion.enabled && i as usize <= head && head - (i as usize) < 4
                    };
                    Span::styled(
                        "█",
                        Style::default().fg(if filled { color } else { p.panel }),
                    )
                })
                .collect::<Vec<_>>();
            f.render_widget(
                Paragraph::new(Line::from(spans)),
                Rect::new(body.x + 2, y + 1, width, 1),
            );
            if body.width > width + 4 {
                let status = if j.done && !j.error {
                    format!(
                        "✓ Saved → {}",
                        j.paths.last().map(|path| shorten(path)).unwrap_or_default()
                    )
                } else {
                    j.status.clone()
                };
                f.render_widget(
                    Paragraph::new(status).style(Style::default().fg(p.muted)),
                    Rect::new(body.x + width + 4, y + 1, body.width - width - 4, 1),
                );
            }
        }
    }
    fn draw_convert(&mut self, f: &mut Frame, area: Rect, p: TeaColors, form: &Form) {
        let dialog = popup(area, 92, 35);
        f.render_widget(Clear, dialog);
        f.render_widget(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(p.primary))
                .style(Style::default().bg(p.background)),
            dialog,
        );
        let inner = inset(dialog, 3, 2);
        let source = if self.page == Page::Files {
            self.files
                .get(self.selected)
                .map(|file| {
                    file.path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                })
                .unwrap_or_default()
        } else {
            self.info
                .as_ref()
                .and_then(|i| i["title"].as_str())
                .unwrap_or("")
                .into()
        };
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("Convert  ", Style::default().bold()),
                Span::styled(source, Style::default().fg(p.muted)),
            ])),
            Rect::new(inner.x, inner.y, inner.width, 1),
        );
        let kind = field_value(form, "kind");
        let mut x = inner.x;
        let kinds = self.kind_choices();
        for k in &kinds {
            let r = Rect::new(x, inner.y + 2, 9.min(inner.right().saturating_sub(x)), 1);
            f.render_widget(
                Paragraph::new(format!(" {} ", title_case(k))).style(if kind == k.as_str() {
                    Style::default().fg(p.primary).bold()
                } else {
                    Style::default().fg(p.muted)
                }),
                r,
            );
            self.hit(r, UiAction::Value(0, k.clone()));
            x += 10;
        }
        if inner.height < 13 {
            self.draw_compact_form(f, inner, p, form);
            return;
        }
        let grid_y = inner.y + 4;
        let grid_h = inner.height.saturating_sub(10).min(22);
        let col = (inner.width.saturating_sub(2)) / 2;
        let top_h = (grid_h.saturating_sub(1)) / 2;
        let bottom_h = grid_h.saturating_sub(top_h + 1);
        let panels = [
            Rect::new(inner.x, grid_y, col, top_h),
            Rect::new(
                inner.x + col + 2,
                grid_y,
                inner.width.saturating_sub(col + 2),
                top_h,
            ),
            Rect::new(inner.x, grid_y + top_h + 1, col, bottom_h),
            Rect::new(
                inner.x + col + 2,
                grid_y + top_h + 1,
                inner.width.saturating_sub(col + 2),
                bottom_h,
            ),
        ];
        for (i, title) in [
            "Format",
            if kind == "image" {
                "Size"
            } else if kind == "frames" {
                "Rate"
            } else {
                "Quality"
            },
            if kind == "image" { "Max size" } else { "Clip" },
            "Extras",
        ]
        .iter()
        .enumerate()
        {
            f.render_widget(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .title(Span::styled(*title, Style::default().fg(p.muted).bold()))
                    .border_style(Style::default().fg(p.panel)),
                panels[i],
            );
        }
        let formats = self.format_choices(kind);
        let selected_format = field_value(form, "format");
        let format_rows = formats
            .iter()
            .map(|fmt| {
                let note =
                    media::tables()[if kind == "frames" { "image" } else { kind }][fmt]["note"]
                        .as_str()
                        .unwrap_or("");
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{:<5} ", fmt.to_uppercase()),
                        Style::default().bold(),
                    ),
                    Span::styled(note.to_owned(), Style::default().fg(p.muted)),
                ]))
            })
            .collect::<Vec<_>>();
        let mut state =
            ListState::default().with_selected(formats.iter().position(|v| v == selected_format));
        let r = inset(panels[0], 2, 1);
        f.render_stateful_widget(
            List::new(format_rows).highlight_style(if form.selected == 1 {
                Style::default().bg(p.primary).fg(p.background)
            } else {
                Style::default().bg(p.panel)
            }),
            r,
            &mut state,
        );
        let offset = state.offset();
        for i in 0..r.height.min(formats.len().saturating_sub(offset) as u16) {
            self.hit(
                Rect::new(r.x, r.y + i, r.width, 1),
                UiAction::Value(1, formats[offset + i as usize].clone()),
            );
        }
        let qualities = self.quality_choices(kind, selected_format);
        let r = inset(panels[1], 2, 1);
        let rows = qualities
            .iter()
            .map(|q| ListItem::new(quality_label(kind, q, self.page == Page::Files)))
            .collect::<Vec<_>>();
        let mut state = ListState::default().with_selected(
            qualities
                .iter()
                .position(|v| v == field_value(form, "quality")),
        );
        f.render_stateful_widget(
            List::new(rows).highlight_style(if form.selected == 2 {
                Style::default().bg(p.primary).fg(p.background)
            } else {
                Style::default().bg(p.panel)
            }),
            r,
            &mut state,
        );
        let offset = state.offset();
        for i in 0..r.height.min(qualities.len().saturating_sub(offset) as u16) {
            self.hit(
                Rect::new(r.x, r.y + i, r.width, 1),
                UiAction::Value(2, qualities[offset + i as usize].clone()),
            );
        }
        let r = inset(panels[2], 2, 2);
        if ["video", "audio"].contains(&kind) {
            let width = r.width.saturating_sub(2) / 2;
            for (i, key) in ["start", "end"].iter().enumerate() {
                let x = r.x + (width + 2) * i as u16;
                f.render_widget(
                    Paragraph::new(label(key)).style(Style::default().fg(p.muted)),
                    Rect::new(x, r.y, width, 1),
                );
                if r.height > 1 {
                    self.field(
                        f,
                        Rect::new(x, r.y + 1, width, 1),
                        field_index(form, key),
                        form,
                        p,
                    );
                }
            }
            if r.height > 3 {
                f.render_widget(
                    Paragraph::new("Whole video if blank\n1:30 · 90 · 1m30s")
                        .style(Style::default().fg(p.muted)),
                    Rect::new(r.x, r.y + 3, r.width, r.height - 3),
                );
            }
        } else if kind == "image" {
            self.field(
                f,
                Rect::new(r.x, r.y, r.width, 1),
                field_index(form, "max_kb"),
                form,
                p,
            );
            if r.height > 2 {
                f.render_widget(
                    Paragraph::new("Blank keeps the current size.")
                        .style(Style::default().fg(p.muted)),
                    Rect::new(r.x, r.y + 2, r.width, r.height - 2),
                );
            }
        } else {
            f.render_widget(
                Paragraph::new("Frames use the whole video.")
                    .style(Style::default().fg(p.muted))
                    .wrap(Wrap { trim: true }),
                r,
            );
        }
        let r = inset(panels[3], 2, 2);
        let visible = self.visible_form_fields(form);
        let extras = visible
            .iter()
            .copied()
            .filter(|i| *i >= 5 && *i != 10)
            .collect::<Vec<_>>();
        for (row, index) in extras.iter().enumerate().take(r.height as usize) {
            let (key, value) = &form.rows[*index];
            let width = 18.min(r.width);
            let y = r.y + row as u16;
            f.render_widget(
                Paragraph::new(label(key).to_owned()).style(Style::default().fg(p.muted)),
                Rect::new(r.x, y, r.width.saturating_sub(width), 1),
            );
            let target = Rect::new(r.right() - width, y, width, 1);
            let selected = form.selected == *index;
            f.render_widget(
                Paragraph::new(display_value(key, value))
                    .right_aligned()
                    .style(
                        Style::default()
                            .fg(if selected { p.primary } else { p.foreground })
                            .bg(if selected { p.panel } else { p.background }),
                    ),
                target,
            );
            self.hit(target, UiAction::Field(*index));
        }
        if extras.is_empty() {
            f.render_widget(
                Paragraph::new("Frames have no extras.").style(Style::default().fg(p.muted)),
                r,
            );
        }
        let summary_y = grid_y + grid_h + 1;
        let quality = field_value(form, "quality");
        f.render_widget(
            Paragraph::new(format!(
                "{} · {}  →  {}",
                selected_format.to_uppercase(),
                quality_label(kind, quality, self.page == Page::Files),
                if self.page == Page::Files {
                    "next to each original".into()
                } else {
                    shorten(&self.settings.download_path())
                }
            ))
            .style(Style::default().fg(p.muted)),
            Rect::new(inner.x, summary_y, inner.width, 1),
        );
        let y = inner.bottom().saturating_sub(3);
        let width = inner.width.saturating_sub(2) / 2;
        self.button(
            f,
            Rect::new(inner.x, y, width, 3),
            if self.page == Page::Files {
                "Convert"
            } else {
                "Download"
            },
            true,
            UiAction::Submit,
            p,
        );
        self.button(
            f,
            Rect::new(
                inner.x + width + 2,
                y,
                inner.width.saturating_sub(width + 2),
                3,
            ),
            "Cancel",
            false,
            UiAction::Close,
            p,
        );
        if !self.status.is_empty() && self.status.contains("must") {
            f.render_widget(
                Paragraph::new(self.status.clone()).style(Style::default().fg(p.error)),
                Rect::new(inner.x, summary_y, inner.width, 1),
            );
        }
    }
    fn draw_compact_form(&mut self, f: &mut Frame, inner: Rect, p: TeaColors, form: &Form) {
        let rows = self.visible_form_fields(form);
        let height = inner.height.saturating_sub(4);
        let start = rows
            .iter()
            .position(|i| *i == form.selected)
            .unwrap_or(0)
            .saturating_sub(height.saturating_sub(1) as usize);
        for (row, i) in rows.iter().skip(start).take(height as usize).enumerate() {
            let (key, _) = &form.rows[*i];
            let width = (inner.width / 2).min(20);
            f.render_widget(
                Paragraph::new(label(key)).style(Style::default().fg(p.muted)),
                Rect::new(inner.x, inner.y + 2 + row as u16, width, 1),
            );
            self.field(
                f,
                Rect::new(
                    inner.x + width,
                    inner.y + 2 + row as u16,
                    inner.width - width,
                    1,
                ),
                *i,
                form,
                p,
            );
        }
        if inner.height > 2 {
            f.render_widget(
                Paragraph::new("Ctrl+S apply · Esc cancel").style(Style::default().fg(p.muted)),
                Rect::new(inner.x, inner.bottom() - 1, inner.width, 1),
            );
        }
    }
    fn draw_settings(&mut self, f: &mut Frame, area: Rect, p: TeaColors, form: &Form) {
        let dialog = popup(area, 76, 29);
        f.render_widget(Clear, dialog);
        f.render_widget(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(p.primary))
                .style(Style::default().bg(p.background)),
            dialog,
        );
        let inner = inset(dialog, 3, 2);
        f.render_widget(
            Paragraph::new(form.title.clone()).bold(),
            Rect::new(inner.x, inner.y, inner.width, 1),
        );
        if inner.height < 21 {
            self.draw_compact_form(f, inner, p, form);
            return;
        }
        for (key, row) in [
            ("download_dir", 2),
            ("cookies_browser", 6),
            ("parallel", 11),
            ("filename_template", 14),
            ("animations", 19),
        ] {
            let index = field_index(form, key);
            f.render_widget(
                Paragraph::new(label(key)).style(Style::default().fg(p.muted)),
                Rect::new(inner.x, inner.y + row, inner.width, 1),
            );
            let width = if key == "download_dir" {
                inner.width.saturating_sub(12)
            } else if ["parallel", "animations"].contains(&key) {
                20.min(inner.width)
            } else {
                inner.width
            };
            self.field(
                f,
                Rect::new(inner.x, inner.y + row + 1, width, 1),
                index,
                form,
                p,
            );
            if key == "download_dir" {
                self.button(
                    f,
                    Rect::new(inner.x + width + 1, inner.y + row + 1, 11, 1),
                    "Browse",
                    false,
                    UiAction::Key('f'),
                    p,
                );
            }
        }
        f.render_widget(Paragraph::new("gingertea reads that browser's YouTube login.\nOn a Mac you may see a Keychain prompt.").style(Style::default().fg(p.muted)),Rect::new(inner.x,inner.y+8,inner.width,2));
        let fields = json!({"title":"Tea and Ginger","id":"dQw4w9Wg","channel":"Ginger Tea","date":"2024-01-02","index":"03","ext":"mp4"});
        let example = state::render_name(field_value(form, "filename_template"), &fields)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "Unknown filename field".into());
        f.render_widget(Paragraph::new(format!("{example}.mp4\n{{title}}  {{id}}  {{channel}}  {{date}}  {{index}}  {{ext}}\n/ makes a folder")).style(Style::default().fg(p.muted)),Rect::new(inner.x,inner.y+16,inner.width,3));
        let y = inner.bottom().saturating_sub(3);
        self.button(
            f,
            Rect::new(inner.x, y, 16, 3),
            "Save",
            true,
            UiAction::Submit,
            p,
        );
        self.button(
            f,
            Rect::new(inner.x + 18, y, 18, 3),
            "Cancel",
            false,
            UiAction::Close,
            p,
        );
    }
    fn draw_list_dialog(&mut self, f: &mut Frame, area: Rect, p: TeaColors) {
        let history = self.page == Page::History;
        let dialog = popup(
            area,
            if history { 80 } else { 64 },
            if history { 32 } else { 30 },
        );
        f.render_widget(Clear, dialog);
        f.render_widget(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(p.primary))
                .style(Style::default().bg(p.background)),
            dialog,
        );
        let r = inset(dialog, 3, 2);
        f.render_widget(
            Paragraph::new(if history {
                "History  enter open · s folder · g again · c copy · x forget"
            } else {
                "Supported sites"
            })
            .style(Style::default().bold()),
            Rect::new(r.x, r.y, r.width, 1),
        );
        let field = Rect::new(r.x, r.y + 2, r.width, 1);
        f.render_widget(
            Paragraph::new(format!(
                " {}",
                if self.input.is_empty() {
                    if history {
                        "Search titles and links"
                    } else {
                        "Search supported sites"
                    }
                } else {
                    &self.input
                }
            ))
            .style(Style::default().bg(p.panel).fg(if self.input.is_empty() {
                p.muted
            } else {
                p.foreground
            })),
            field,
        );
        self.hit(field, UiAction::Input);
        if self.editing {
            f.set_cursor_position((
                field.x
                    + 1
                    + self
                        .input
                        .chars()
                        .count()
                        .min(field.width.saturating_sub(2) as usize) as u16,
                field.y,
            ));
        }
        let rows: Vec<_> = if history {
            self.filtered_history()
                .iter()
                .map(|i| i["title"].as_str().unwrap_or("Untitled").to_owned())
                .collect()
        } else {
            self.sites
                .iter()
                .filter(|site| site.to_lowercase().contains(&self.input.to_lowercase()))
                .cloned()
                .collect()
        };
        let body = Rect::new(r.x, r.y + 4, r.width, r.height.saturating_sub(4));
        let mut state = ListState::default().with_selected(Some(self.selected));
        f.render_stateful_widget(
            List::new(
                rows.iter()
                    .map(|s| ListItem::new(s.as_str()))
                    .collect::<Vec<_>>(),
            )
            .highlight_style(
                Style::default()
                    .bg(if self.editing { p.panel } else { p.primary })
                    .fg(if self.editing {
                        p.foreground
                    } else {
                        p.background
                    }),
            ),
            body,
            &mut state,
        );
        let offset = state.offset();
        for row in 0..body.height.min(rows.len().saturating_sub(offset) as u16) {
            self.hit(
                Rect::new(body.x, body.y + row, body.width, 1),
                UiAction::Select(offset + row as usize),
            );
        }
    }
    fn draw_playlist_dialog(&mut self, f: &mut Frame, area: Rect, p: TeaColors) {
        let dialog = popup(area, 80, 36);
        f.render_widget(Clear, dialog);
        f.render_widget(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(p.primary))
                .style(Style::default().bg(p.background)),
            dialog,
        );
        let r = inset(dialog, 3, 2);
        f.render_widget(
            Paragraph::new(format!(
                "Choose videos        {} videos",
                self.entries.len()
            ))
            .bold(),
            Rect::new(r.x, r.y, r.width, 1),
        );
        f.render_widget(
            Paragraph::new("First").style(Style::default().fg(p.muted)),
            Rect::new(r.x, r.y + 2, 7, 1),
        );
        let first = Rect::new(r.x + 7, r.y + 2, 10, 1);
        f.render_widget(
            Paragraph::new(format!(" {}", self.playlist_first)).style(Style::default().bg(
                if self.playlist_typing {
                    motion::mix(p.background, p.primary, 0.25)
                } else {
                    p.panel
                },
            )),
            first,
        );
        self.hit(first, UiAction::PlaylistFirst);
        f.render_widget(
            Paragraph::new(format!(
                "of {}    empty keeps the ticked rows",
                self.entries.len()
            ))
            .style(Style::default().fg(p.muted)),
            Rect::new(r.x + 18, r.y + 2, r.width.saturating_sub(18), 1),
        );
        let body = Rect::new(r.x, r.y + 4, r.width, r.height.saturating_sub(8));
        let rows = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                ListItem::new(format!(
                    "{}  {:02}  {:<48} {}",
                    if e.selected { "●" } else { "○" },
                    i + 1,
                    e.title.chars().take(48).collect::<String>(),
                    e.detail
                ))
            })
            .collect::<Vec<_>>();
        let mut state = ListState::default().with_selected(Some(self.selected));
        f.render_stateful_widget(
            List::new(rows).highlight_style(Style::default().bg(p.primary).fg(p.background)),
            body,
            &mut state,
        );
        let offset = state.offset();
        for row in 0..body
            .height
            .min(self.entries.len().saturating_sub(offset) as u16)
        {
            self.hit(
                Rect::new(body.x, body.y + row, body.width, 1),
                UiAction::Select(offset + row as usize),
            );
        }
        let y = r.bottom().saturating_sub(3);
        let width = r.width.saturating_sub(2) / 2;
        let count = self.entries.iter().filter(|e| e.selected).count();
        self.button(
            f,
            Rect::new(r.x, y, width, 3),
            &format!(
                "Download {}",
                if count == self.entries.len() {
                    "all".into()
                } else {
                    count.to_string()
                }
            ),
            true,
            UiAction::ChoosePlaylist,
            p,
        );
        self.button(
            f,
            Rect::new(r.x + width + 2, y, r.width - width - 2, 3),
            "Cancel",
            false,
            UiAction::Close,
            p,
        );
    }
}
fn title_case(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
    }
}
fn quality_label(kind: &str, q: &str, local: bool) -> String {
    if q == "best" {
        return if kind == "image" {
            "Original size"
        } else if kind == "frames" {
            "Every frame"
        } else if local && kind == "video" {
            "Same as original"
        } else {
            "Best available"
        }
        .into();
    }
    match kind {
        "image" => {
            if q == "50" {
                "Half size".into()
            } else {
                format!("Up to {q}px")
            }
        }
        "frames" => format!("{q}/s"),
        "audio" => format!("{q} kbps"),
        _ => format!("{q}p"),
    }
}

impl App {
    pub(super) fn kind_choices(&self) -> Vec<String> {
        let kinds = if self.page == Page::Files {
            match self.files.get(self.selected).map(|f| f.media.as_str()) {
                Some("audio") => vec!["audio"],
                Some("image") => vec!["image"],
                _ => vec!["video", "audio", "frames"],
            }
        } else {
            vec!["video", "audio"]
        };
        kinds.into_iter().map(str::to_owned).collect()
    }
    pub(super) fn format_choices(&self, kind: &str) -> Vec<String> {
        let local = self.page == Page::Files;
        let formats = match kind {
            "video" if !local => vec!["mp4", "mkv", "webm", "mov", "gif"],
            "video" => vec![
                "mp4", "mkv", "webm", "mov", "m4v", "avi", "ts", "flv", "wmv", "mpg", "3gp", "ogv",
                "gif", "apng", "webp",
            ],
            "audio" if !local => vec!["mp3", "m4a", "opus", "flac", "wav", "ogg"],
            "audio" => vec![
                "mp3", "m4a", "aac", "opus", "ogg", "flac", "alac", "wav", "aiff", "wma", "ac3",
                "eac3", "mp2", "wv", "tta", "caf", "au", "mka",
            ],
            _ => vec![
                "jpg", "png", "webp", "avif", "jxl", "heic", "jp2", "tiff", "bmp", "gif", "tga",
                "qoi", "exr", "hdr", "ppm", "pcx", "sgi", "dpx",
            ],
        };
        formats
            .into_iter()
            .filter(|f| kind != "frames" || !["jxl", "heic"].contains(f))
            .map(str::to_owned)
            .collect()
    }
    pub(super) fn quality_choices(&self, kind: &str, fmt: &str) -> Vec<String> {
        let choices = match kind {
            "audio" => {
                if [
                    "flac", "wav", "alac", "aiff", "wv", "tta", "caf", "au", "mka",
                ]
                .contains(&fmt)
                {
                    vec!["best"]
                } else {
                    vec!["best", "320", "192", "128"]
                }
            }
            "image" => vec!["best", "50", "3840", "1920", "1280"],
            "frames" => vec!["best", "10", "1", "0.1"],
            _ if ["gif", "apng", "webp"].contains(&fmt) => vec!["480", "360", "240"],
            _ => vec!["best", "2160", "1440", "1080", "720", "480", "360"],
        };
        choices.into_iter().map(str::to_owned).collect()
    }
    pub(super) fn visible_form_fields(&self, form: &Form) -> Vec<usize> {
        if form.purpose != "convert" {
            return (0..form.rows.len()).collect();
        }
        let kind = field_value(form, "kind");
        let keys = match kind {
            "image" => vec!["kind", "format", "quality", "max_kb", "strip_exif"],
            "frames" => vec!["kind", "format", "quality"],
            "audio" => vec![
                "kind", "format", "quality", "start", "end", "chapters", "loudnorm",
            ],
            _ if self.page == Page::Files => vec![
                "kind", "format", "quality", "start", "end", "chapters", "loudnorm",
            ],
            _ => vec![
                "kind",
                "format",
                "quality",
                "start",
                "end",
                "subtitles",
                "languages",
                "sponsorblock",
                "chapters",
                "loudnorm",
            ],
        };
        keys.iter().map(|key| field_index(form, key)).collect()
    }
    pub(super) fn form_choices(&self, key: &str) -> Vec<String> {
        let Some(form) = &self.form else {
            return vec![];
        };
        match key {
            "kind" => self.kind_choices(),
            "format" => self.format_choices(field_value(form, "kind")),
            "quality" => {
                self.quality_choices(field_value(form, "kind"), field_value(form, "format"))
            }
            "subtitles" => vec!["off".into(), "embed".into(), "srt".into()],
            "sponsorblock" => vec!["off".into(), "cut".into(), "chapters".into()],
            "parallel" => vec!["1".into(), "2".into(), "3".into(), "4".into()],
            "cookies_browser" => [
                "", "chrome", "firefox", "safari", "edge", "brave", "opera", "vivaldi", "chromium",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            "animations" | "chapters" | "loudnorm" | "strip_exif" => {
                vec!["false".into(), "true".into()]
            }
            _ => vec![],
        }
    }
    pub(super) fn set_form_value(&mut self, index: usize, value: String) {
        if let Some(form) = &mut self.form {
            form.selected = index;
            form.rows[index].1 = value.clone();
            if form.rows[index].0 == "kind" {
                for (key, v) in &mut form.rows {
                    if key == "format" {
                        *v = match value.as_str() {
                            "audio" => "mp3",
                            "image" => "jpg",
                            "frames" => "png",
                            _ => "mp4",
                        }
                        .into()
                    } else if key == "quality" {
                        *v = if value == "frames" { "1" } else { "best" }.into()
                    }
                }
            }
        }
    }
    pub(super) fn mouse(&mut self, event: MouseEvent) -> Result<()> {
        let point = Position::new(event.column, event.row);
        let action = self
            .hitboxes
            .iter()
            .rev()
            .find(|(r, _)| r.contains(point))
            .map(|(_, a)| a.clone());
        if event.kind == MouseEventKind::Moved {
            if self.hovered != action {
                if let Some(UiAction::Theme(index)) = action.as_ref() {
                    self.preview_theme(*index);
                }
                self.hovered = action;
                self.dirty = true;
            }
            return Ok(());
        }
        if matches!(
            event.kind,
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp
        ) {
            let code = if event.kind == MouseEventKind::ScrollDown {
                KeyCode::Down
            } else {
                KeyCode::Up
            };
            let editing = self.editing;
            self.editing = false;
            self.key(KeyEvent::new(code, KeyModifiers::NONE))?;
            self.editing = editing;
            self.dirty = true;
            return Ok(());
        }
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            return Ok(());
        }
        let Some(action) = action else { return Ok(()) };
        match action {
            UiAction::Key('\t') => self.editing = false,
            UiAction::Key('f') if self.form.as_ref().is_some_and(|f| f.purpose == "settings") => {
                self.folder_pick = true;
                self.open_browser(std::env::current_dir()?)?;
            }
            UiAction::Key(c) => {
                self.editing = false;
                if self.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))? {
                    self.quit_requested = true;
                }
            }
            UiAction::Input => self.editing = true,
            UiAction::Submit => self.submit_form()?,
            UiAction::Close => {
                if self.form.is_some() {
                    self.form = None;
                } else if self.choosing {
                    self.cancel_selection();
                } else {
                    self.page = self.return_page;
                    self.input.clear();
                    self.editing = false;
                }
            }
            UiAction::Field(index) => {
                if let Some(form) = &mut self.form {
                    form.selected = index;
                }
            }
            UiAction::Value(index, value) => self.set_form_value(index, value),
            UiAction::CancelJob(i) => {
                if let Some(j) = self.jobs.get_mut(i) {
                    j.cancel.store(true, Ordering::Relaxed);
                    if !j.started {
                        j.done = true;
                        j.status = "Cancelled".into();
                    }
                }
            }
            UiAction::Select(i) => {
                self.selected = i;
                self.editing = false;
                if self.choosing
                    && let Some(e) = self.entries.get_mut(i)
                {
                    e.selected = !e.selected;
                }
            }
            UiAction::Theme(i) => {
                self.preview_theme(i);
                self.finish_theme(true);
            }
            UiAction::ChoosePlaylist => self.download_selection(),
            UiAction::PlaylistFirst => self.playlist_typing = true,
            UiAction::Retry => {
                let input = self.input.clone();
                self.load_input(input);
            }
            UiAction::Browser(i) => {
                if let Some(browser) = &mut self.browser {
                    browser.selected = i;
                }
                self.browser_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
            }
        }
        self.dirty = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_workspace_positions() {
        let r = Workspace::new(Rect::new(0, 0, 120, 46), 5, 3);
        assert_eq!(r.logo, Rect::new(2, 1, 116, 6));
        assert_eq!(r.status.y, 8);
        assert_eq!(r.tabs.y, 10);
        assert_eq!(r.input.y, 13);
        assert_eq!(r.card.y, 17);
        assert_eq!(r.queue.y, 23);
        assert_eq!(r.footer.y, 43);
    }
    #[test]
    fn small_terminals_keep_controls_in_bounds() {
        for (w, h) in [(20, 12), (60, 24), (80, 30), (120, 46)] {
            let area = Rect::new(0, 0, w, h);
            let r = Workspace::new(area, 13, 3);
            for rect in [r.logo, r.status, r.tabs, r.input, r.card, r.queue, r.footer] {
                assert!(rect.right() <= w && rect.bottom() <= h, "{rect:?}");
            }
            if h < 30 {
                assert_eq!(r.logo.height, 0);
            }
        }
    }
    #[test]
    fn footer_matches_original_wrapping() {
        let k = footer_keys(false, true);
        let r = footer_lines(&k, 120);
        assert_eq!(r.len(), 2);
        assert_eq!(k[*r[0].last().unwrap()].0, "s");
    }
}
