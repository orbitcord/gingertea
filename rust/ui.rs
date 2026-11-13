use crate::{
    banner::{self, TeaColors},
    download,
    media::{self, Cancel, Choice, FileInfo, Reporter},
    motion::{self, Motion},
    state::{self, Settings},
};
use anyhow::Result;
use crossterm::{
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
        MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use serde_json::{Value, json};
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
const THEMES: &[&str] = &[
    "dusk",
    "earl-grey",
    "gingertea",
    "gingertea-light",
    "hibiscus",
    "masala-chai",
    "matcha",
    "tea-house-dark",
    "tea-house",
    "tide",
];
fn theme_label(id: &str) -> &str {
    match id {
        "tea-house-dark" => "tea-house",
        "tea-house" => "tea-house-light",
        _ => id,
    }
}
#[derive(Clone)]
struct Entry {
    url: String,
    title: String,
    detail: String,
    selected: bool,
}
struct Job {
    folder: String,
    title: String,
    input: String,
    choice: Choice,
    local: bool,
    status: String,
    done: bool,
    started: bool,
    cancel: Cancel,
    paths: Vec<PathBuf>,
    error: bool,
    fraction: Option<f64>,
}
enum Message {
    Probe(u64, Result<Value, String>),
    Files(Result<Vec<FileInfo>, String>),
    Progress(usize, String),
    Finished(usize, Result<Vec<PathBuf>, String>),
    Sites(Result<Vec<String>, String>),
    Thumb(u64, Vec<Vec<(u8, u8, u8)>>),
    Update(Result<(), String>),
    BackendVersion(String),
}
#[derive(Clone, Copy, PartialEq)]
enum Page {
    Download,
    Files,
    History,
    Sites,
}
#[derive(Clone)]
struct Form {
    title: String,
    rows: Vec<(String, String)>,
    selected: usize,
    purpose: String,
    cursor: Option<usize>,
}
struct Browser {
    dir: PathBuf,
    entries: Vec<PathBuf>,
    selected: usize,
}
#[derive(Clone, PartialEq)]
enum UiAction {
    Key(char),
    Input,
    Submit,
    Close,
    Field(usize),
    Value(usize, String),
    CancelJob(usize),
    Select(usize),
    Theme(usize),
    ChoosePlaylist,
    PlaylistFirst,
    PlaylistAll(bool),
    DropdownValue(usize, String),
    Retry,
    Browser(usize),
}
struct App {
    settings: Settings,
    page: Page,
    input: String,
    input_cursor: Option<usize>,
    focus: Option<UiAction>,
    focusable_buttons: Vec<UiAction>,
    pending_url: Option<(Instant, String)>,
    dropdown: Option<(usize, Vec<String>, usize)>,
    editing: bool,
    selected: usize,
    entries: Vec<Entry>,
    files: Vec<FileInfo>,
    jobs: Vec<Job>,
    paused: bool,
    status: String,
    history: Vec<Value>,
    sites: Vec<String>,
    form: Option<Form>,
    browser: Option<Browser>,
    info: Option<Value>,
    thumb: Option<Vec<Vec<(u8, u8, u8)>>>,
    tx: mpsc::Sender<Message>,
    rx: mpsc::Receiver<Message>,
    generation: u64,
    dirty: bool,
    last_saved: Option<PathBuf>,
    last_frame: Instant,
    motion: Motion,
    loading_video: Option<u64>,
    loading_files: usize,
    loading_sites: bool,
    updating: bool,
    hitboxes: Vec<(Rect, UiAction)>,
    hovered: Option<UiAction>,
    return_page: Page,
    choosing: bool,
    theme_picker: Option<usize>,
    theme_original: Option<String>,
    theme_scroll: usize,
    toast: Option<Instant>,
    last_status: String,
    backend_version: String,
    probe_error: Option<String>,
    quit_requested: bool,
    folder_pick: bool,
    playlist_first: String,
    playlist_typing: bool,
    playlist_backup: Vec<bool>,
}
fn local_kinds(file: &FileInfo) -> &'static [&'static str] {
    match file.media.as_str() {
        "image" => &["image"],
        "audio" => &["audio"],
        _ if file
            .path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("gif")) =>
        {
            &["video", "image"]
        }
        _ if file.audio.is_some() => &["video", "audio", "frames"],
        _ => &["video", "frames"],
    }
}
struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            DisableBracketedPaste,
            DisableMouseCapture,
            LeaveAlternateScreen
        );
    }
}
pub fn run(settings: Settings, initial: String, cancel: Cancel) -> Result<()> {
    // This interactive application explicitly renders a theme. CLI commands keep
    // their normal NO_COLOR behavior; do not mutate the parent environment.
    ratatui::crossterm::style::force_color_output(true);
    crossterm::style::force_color_output(true);
    enable_raw_mode()?;
    execute!(
        io::stdout(),
        EnterAlternateScreen,
        EnableBracketedPaste,
        EnableMouseCapture
    )?;
    let _guard = TerminalGuard;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let (tx, rx) = mpsc::channel();
    let now = Instant::now();
    let motion = Motion::new(
        settings.animations && std::env::var_os("GINGERTEA_NO_ANIMATION").is_none(),
        now,
    );
    let mut app = App {
        settings,
        page: Page::Download,
        input: String::new(),
        input_cursor: None,
        focus: None,
        focusable_buttons: Vec::new(),
        pending_url: None,
        dropdown: None,
        editing: true,
        selected: 0,
        entries: Vec::new(),
        files: Vec::new(),
        jobs: Vec::new(),
        paused: false,
        status: "Paste a link, search, or drop files. Tab enables shortcuts.".into(),
        history: state::history(),
        sites: Vec::new(),
        form: None,
        browser: None,
        info: None,
        thumb: None,
        tx,
        rx,
        generation: 0,
        dirty: true,
        last_saved: None,
        last_frame: now,
        motion,
        loading_video: None,
        loading_files: 0,
        loading_sites: false,
        updating: false,
        hitboxes: Vec::new(),
        hovered: None,
        return_page: Page::Download,
        choosing: false,
        theme_picker: None,
        theme_original: None,
        theme_scroll: 0,
        toast: None,
        last_status: String::new(),
        backend_version: String::new(),
        probe_error: None,
        quit_requested: false,
        folder_pick: false,
        playlist_first: String::new(),
        playlist_typing: false,
        playlist_backup: Vec::new(),
    };
    {
        let tx = app.tx.clone();
        thread::spawn(move || {
            let mut cmd = media::command("yt-dlp");
            cmd.arg("--version");
            let _ = tx.send(Message::BackendVersion(
                media::capture(cmd).unwrap_or_default().trim().to_owned(),
            ));
        });
    }
    if !initial.is_empty() {
        app.load_input(initial)
    }
    let mut was_animating = false;
    loop {
        if cancel.load(Ordering::Relaxed) || app.quit_requested {
            break;
        }
        if app
            .pending_url
            .as_ref()
            .is_some_and(|(at, _)| at.elapsed() >= Duration::from_millis(350))
            && app.form.is_none()
            && let Some((_, url)) = app.pending_url.take()
            && url == app.input
        {
            app.load_input(url);
        }
        app.messages();
        app.schedule();
        if app.status != app.last_status {
            app.last_status = app.status.clone();
            app.toast = if app.status.starts_with("Paste a link") || app.status.contains("item(s)")
            {
                None
            } else {
                Some(Instant::now())
            };
        }
        if app
            .toast
            .is_some_and(|at| at.elapsed() >= Duration::from_secs(3))
        {
            app.toast = None;
            app.dirty = true;
        }
        app.motion.enabled =
            app.settings.animations && std::env::var_os("GINGERTEA_NO_ANIMATION").is_none();
        let interval = app.motion.interval(Instant::now(), app.busy());
        if interval.is_some_and(|interval| app.last_frame.elapsed() >= interval)
            || was_animating && interval.is_none()
        {
            app.dirty = true;
        }
        was_animating = interval.is_some();
        if app.dirty {
            terminal.draw(|f| app.draw(f))?;
            app.dirty = false;
            app.last_frame = Instant::now();
        }
        let wait = interval
            .map(|interval| {
                interval
                    .saturating_sub(app.last_frame.elapsed())
                    .min(Duration::from_millis(40))
            })
            .unwrap_or(Duration::from_millis(40));
        if event::poll(wait)? {
            match event::read()? {
                Event::Key(k) if matches!(k.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                    if app.key(k)? {
                        break;
                    }
                    app.dirty = true
                }
                Event::Paste(s) => {
                    app.motion.pasted = Some(Instant::now());
                    if let Some(form) = app.form.as_ref() {
                        if app.form_choices(&form.rows[form.selected].0).is_empty()
                            && app.focus == Some(UiAction::Field(form.selected))
                        {
                            let form = app.form.as_mut().unwrap();
                            for ch in s.chars().filter(|ch| !ch.is_control()) {
                                crate::text_input::edit(
                                    &mut form.rows[form.selected].1,
                                    &mut form.cursor,
                                    KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
                                );
                            }
                        }
                    } else {
                        app.load_input(s);
                    }
                    app.dirty = true
                }
                Event::Mouse(event) => {
                    app.mouse(event)?;
                }
                Event::Resize(..) => app.dirty = true,
                _ => (),
            }
        }
    }
    media::shutdown_workers();
    for job in &app.jobs {
        job.cancel.store(true, Ordering::Relaxed);
    } // allow workers to reap media children before terminal teardown
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.jobs.iter().any(|j| j.started && !j.done) && Instant::now() < deadline {
        app.messages();
        thread::sleep(Duration::from_millis(30));
    }
    Ok(())
}
impl App {
    fn cancel_selection(&mut self) {
        self.choosing = false;
        for (e, selected) in self.entries.iter_mut().zip(&self.playlist_backup) {
            e.selected = *selected;
        }
    }
    fn select_all_playlist(&mut self, selected: bool) {
        for entry in &mut self.entries {
            entry.selected = selected;
        }
        self.playlist_first.clear();
        self.playlist_typing = false;
        self.dirty = true;
    }
    fn download_selection(&mut self) {
        if let Ok(n) = self.playlist_first.parse::<usize>()
            && n > 0
        {
            for (i, e) in self.entries.iter_mut().enumerate() {
                e.selected = i < n;
            }
        }
        if self.entries.iter().any(|e| e.selected) {
            self.choosing = false;
            self.enqueue(Choice::default());
        }
    }
    fn preview_theme(&mut self, index: usize) {
        let index = index.min(THEMES.len() - 1);
        self.theme_picker = Some(index);
        self.settings.theme = THEMES[index].into();
        self.dirty = true;
    }
    fn finish_theme(&mut self, save: bool) {
        if save {
            if let Err(error) = self.settings.save() {
                self.status = format!("Could not save theme: {error}");
                return;
            }
        } else if let Some(original) = self.theme_original.as_ref() {
            self.settings.theme = original.clone();
        }
        self.theme_original = None;
        self.theme_picker = None;
        self.dirty = true;
    }
    fn busy(&self) -> bool {
        self.loading_video.is_some()
            || self.loading_files > 0
            || self.loading_sites
            || self.updating
            || self.jobs.iter().any(|j| j.started && !j.done)
    }
    fn load_input(&mut self, text: String) {
        self.pending_url = None;
        self.input_cursor = None;
        let trimmed = text.trim();
        let whole = state::expand(trimmed.trim_matches(['\'', '"']));
        let paths = if whole.is_file() {
            vec![whole]
        } else {
            shell_words::split(trimmed)
                .unwrap_or_default()
                .iter()
                .map(|p| state::expand(&decode_file_url(p)))
                .filter(|p| p.is_file())
                .collect()
        };
        if !paths.is_empty() {
            self.page = Page::Files;
            self.status = "Reading files…".into();
            self.loading_files += 1;
            self.loading_video = None;
            self.generation += 1;
            let tx = self.tx.clone();
            thread::spawn(move || {
                let result = paths
                    .iter()
                    .map(|p| media::probe(p))
                    .collect::<Result<Vec<_>>>()
                    .map_err(|e| format!("{e:#}"));
                let _ = tx.send(Message::Files(result));
            });
            self.input.clear();
        } else {
            self.page = Page::Download;
            self.input = trimmed.into();
            self.thumb = None;
            self.info = None;
            self.probe_error = None;
            self.entries.clear();
            self.generation += 1;
            let generation = self.generation;
            self.loading_video = Some(generation);
            let tx = self.tx.clone();
            let s = self.settings.clone();
            let input = self.input.clone();
            self.status = "Getting video details…".into();
            thread::spawn(move || {
                let r = download::probe(&input, &s).map_err(|e| format!("{e:#}"));
                let _ = tx.send(Message::Probe(generation, r));
            });
        }
    }
    fn messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            self.dirty = true;
            match msg {
                Message::Probe(generation, result) => {
                    if generation != self.generation {
                        continue;
                    }
                    self.loading_video = None;
                    match result {
                        Ok(info) => {
                            self.motion.ready = Some(Instant::now());
                            self.selected = 0;
                            self.entries = if let Some(entries) = info["entries"].as_array() {
                                entries
                                    .iter()
                                    .map(|v| Entry {
                                        url: v["webpage_url"]
                                            .as_str()
                                            .or(v["url"].as_str())
                                            .unwrap_or("")
                                            .into(),
                                        title: v["title"].as_str().unwrap_or("Untitled").into(),
                                        detail: format!(
                                            "{}  {}",
                                            v["channel"].as_str().unwrap_or(""),
                                            duration(media::number(&v["duration"]))
                                        ),
                                        selected: true,
                                    })
                                    .collect()
                            } else {
                                vec![Entry {
                                    url: info["webpage_url"].as_str().unwrap_or(&self.input).into(),
                                    title: info["title"].as_str().unwrap_or("Untitled").into(),
                                    detail: format!(
                                        "{}  {}",
                                        info["channel"].as_str().unwrap_or(""),
                                        duration(media::number(&info["duration"]))
                                    ),
                                    selected: true,
                                }]
                            };
                            self.status = format!(
                                "{} item(s). d Download · a Audio · c Options · Space Select",
                                self.entries.len()
                            );
                            if let Some(url) = info["thumbnail"].as_str() {
                                let url = url.to_owned();
                                let tx = self.tx.clone();
                                thread::spawn(move || {
                                    if let Ok(t) = media::thumbnail(&url) {
                                        let _ = tx.send(Message::Thumb(generation, t));
                                    }
                                });
                            }
                            self.info = Some(info);
                        }
                        Err(e) => {
                            self.probe_error = Some(e.clone());
                            self.status = e;
                        }
                    }
                }
                Message::Files(result) => {
                    self.loading_files = self.loading_files.saturating_sub(1);
                    match result {
                        Ok(files) => {
                            self.motion.ready = Some(Instant::now());
                            for file in files {
                                if !self.files.iter().any(|f| f.path == file.path) {
                                    self.files.push(file)
                                }
                            }
                            self.status = "Files ready. c Convert · f Browse · r Remove".into();
                        }
                        Err(e) => self.status = e,
                    }
                }
                Message::Progress(i, s) => {
                    if let Some(j) = self.jobs.get_mut(i) {
                        j.fraction = motion::percentage(&s);
                        j.status = s
                    }
                }
                Message::Finished(i, result) => {
                    if let Some(j) = self.jobs.get_mut(i) {
                        j.done = true;
                        self.motion.finished = Some((Instant::now(), result.is_ok()));
                        match result {
                            Ok(paths) => {
                                self.last_saved = paths.last().cloned();
                                j.paths = paths;
                                j.status = "Done".into();
                            }
                            Err(e) => {
                                j.error = true;
                                j.status = e;
                            }
                        }
                    }
                    self.history = state::history();
                    if self.jobs.iter().all(|j| j.done) {
                        self.status = "Queue finished".into();
                        notify("Queue finished");
                    }
                }
                Message::Sites(result) => {
                    self.loading_sites = false;
                    match result {
                        Ok(sites) => self.sites = sites,
                        Err(e) => self.status = e,
                    }
                }
                Message::Thumb(generation, t) => {
                    if generation == self.generation {
                        self.thumb = Some(t)
                    }
                }
                Message::BackendVersion(version) => self.backend_version = version,
                Message::Update(r) => {
                    self.updating = false;
                    self.status = match r {
                        Ok(()) => "yt-dlp updated".into(),
                        Err(e) => e,
                    }
                }
            }
        }
    }
    fn enqueue(&mut self, choice: Choice) {
        match self.page {
            Page::Download => {
                for e in self.entries.iter().filter(|e| e.selected) {
                    self.jobs.push(Job {
                        folder: if self.info.as_ref().is_some_and(|v| v["entries"].is_array()) {
                            state::sanitize(
                                self.info
                                    .as_ref()
                                    .and_then(|v| v["title"].as_str())
                                    .unwrap_or("playlist"),
                            )
                        } else {
                            String::new()
                        },
                        title: e.title.clone(),
                        input: e.url.clone(),
                        choice: choice.clone(),
                        local: false,
                        status: "Waiting".into(),
                        done: false,
                        started: false,
                        cancel: Arc::new(AtomicBool::new(false)),
                        paths: Vec::new(),
                        error: false,
                        fraction: None,
                    });
                }
            }
            Page::Files => {
                for f in &self.files {
                    if !local_kinds(f).contains(&choice.kind.as_str()) {
                        continue;
                    }
                    self.jobs.push(Job {
                        title: f
                            .path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into(),
                        input: f.path.to_string_lossy().into(),
                        choice: choice.clone(),
                        folder: String::new(),
                        local: true,
                        status: "Waiting".into(),
                        done: false,
                        started: false,
                        cancel: Arc::new(AtomicBool::new(false)),
                        paths: Vec::new(),
                        error: false,
                        fraction: None,
                    });
                }
            }
            _ => (),
        }
        self.status = format!("{} jobs in queue", self.jobs.len());
    }
    fn schedule(&mut self) {
        if self.paused {
            return;
        }
        let active = self.jobs.iter().filter(|j| j.started && !j.done).count();
        let available = self.settings.parallel.saturating_sub(active);
        let indices: Vec<_> = self
            .jobs
            .iter()
            .enumerate()
            .filter(|(_, j)| !j.started && !j.done)
            .map(|(i, _)| i)
            .take(available)
            .collect();
        for i in indices {
            let j = &mut self.jobs[i];
            j.started = true;
            j.status = "Getting ready".into();
            let input = j.input.clone();
            let c = j.choice.clone();
            let local = j.local;
            let cancel = j.cancel.clone();
            let mut s = self.settings.clone();
            let use_archive = !j.folder.is_empty();
            if use_archive {
                s.download_dir = s.download_path().join(&j.folder).to_string_lossy().into();
            }
            let tx = self.tx.clone();
            let progress_tx = tx.clone();
            let duration = self
                .files
                .iter()
                .find(|f| f.path == Path::new(&input))
                .map(|f| f.duration)
                .unwrap_or(0.);
            let duration = c.end.unwrap_or(duration) - c.start.unwrap_or(0.);
            let report: Reporter = Arc::new(move |line| {
                let line = if local && line.contains('=') {
                    if let Some(value) = line.strip_prefix("out_time_us=") {
                        if duration > 0. {
                            format!(
                                "Converting {:.0}%",
                                (value.parse::<f64>().unwrap_or(0.) / 1_000_000. / duration * 100.)
                                    .clamp(0., 100.)
                            )
                        } else {
                            "Converting".into()
                        }
                    } else {
                        return;
                    }
                } else {
                    line
                };
                let _ = progress_tx.send(Message::Progress(i, line));
            });
            thread::spawn(move || {
                let result = (|| -> Result<Vec<PathBuf>> {
                    if local {
                        let info = media::probe(Path::new(&input))?;
                        let out = media::convert(&info, &c, &cancel, &report)?;
                        state::remember("", &input, &out)?;
                        Ok(vec![out])
                    } else {
                        download::download_item(
                            &input,
                            &c,
                            &s,
                            &cancel,
                            &report,
                            false,
                            use_archive,
                        )
                    }
                })()
                .map_err(|e| format!("{e:#}"));
                let _ = tx.send(Message::Finished(i, result));
            });
            self.dirty = true;
        }
    }
    fn options(&mut self) {
        let mut c = Choice::saved(&self.settings);
        let kinds = self.kind_choices();
        if !kinds.contains(&c.kind) {
            c.kind = kinds.first().cloned().unwrap_or_else(|| "video".into());
            let mut saved = self.settings.clone();
            saved.last_kind = c.kind.clone();
            c = Choice::saved(&saved);
        }
        let rows = vec![
            ("kind".into(), c.kind),
            ("format".into(), c.fmt),
            ("quality".into(), c.quality),
            ("start".into(), String::new()),
            ("end".into(), String::new()),
            ("subtitles".into(), c.subtitles),
            ("languages".into(), c.languages),
            ("sponsorblock".into(), c.sponsorblock),
            ("chapters".into(), c.chapters.to_string()),
            ("loudnorm".into(), c.loudnorm.to_string()),
            (
                "max_kb".into(),
                c.max_kb.map(|v| v.to_string()).unwrap_or_default(),
            ),
            ("strip_exif".into(), c.strip_exif.to_string()),
        ];
        self.focus = Some(UiAction::Field(1));
        self.form = Some(Form {
            title: "Convert options · ↑↓ field · ←→ choices · Ctrl+U clear · Ctrl+S Run".into(),
            rows,
            selected: 1,
            purpose: "convert".into(),
            cursor: None,
        });
        self.normalize_convert();
    }
    fn settings_form(&mut self) {
        let value = serde_json::to_value(&self.settings).unwrap();
        let rows = [
            "download_dir",
            "cookies_browser",
            "parallel",
            "filename_template",
            "animations",
        ]
        .iter()
        .map(|key| {
            let v = &value[*key];
            (
                key.to_string(),
                v.as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| v.to_string()),
            )
        })
        .collect();
        self.focus = Some(UiAction::Field(0));
        self.form = Some(Form {
            title: "Settings".into(),
            rows,
            selected: 0,
            purpose: "settings".into(),
            cursor: None,
        });
    }
    fn submit_form(&mut self) -> Result<()> {
        let form = self.form.as_ref().unwrap().clone();
        let map: std::collections::HashMap<_, _> = form.rows.iter().cloned().collect();
        let value = |key: &str| map.get(key).cloned().unwrap_or_default();
        let result = (|| -> Result<()> {
            if form.purpose == "convert" {
                let c = Choice {
                    kind: value("kind"),
                    fmt: value("format"),
                    quality: value("quality"),
                    start: state::parse_time(&value("start"))?,
                    end: state::parse_time(&value("end"))?,
                    subtitles: value("subtitles"),
                    languages: value("languages"),
                    sponsorblock: value("sponsorblock"),
                    chapters: value("chapters") == "true",
                    loudnorm: value("loudnorm") == "true",
                    max_kb: if value("max_kb").is_empty() {
                        None
                    } else {
                        Some(value("max_kb").parse()?)
                    },
                    strip_exif: value("strip_exif") == "true",
                };
                c.validate()?;
                self.settings.last_kind = c.kind.clone();
                match c.kind.as_str() {
                    "audio" => {
                        self.settings.last_audio_format = c.fmt.clone();
                        self.settings.last_audio_quality = c.quality.clone()
                    }
                    "image" => {
                        self.settings.last_image_format = c.fmt.clone();
                        self.settings.last_image_quality = c.quality.clone()
                    }
                    "frames" => {
                        self.settings.last_frames_format = c.fmt.clone();
                        self.settings.last_frames_quality = c.quality.clone()
                    }
                    _ => {
                        self.settings.last_video_format = c.fmt.clone();
                        self.settings.last_video_quality = c.quality.clone()
                    }
                }
                self.settings.subtitle_mode = c.subtitles.clone();
                self.settings.subtitle_langs = c.languages.clone();
                self.settings.sponsorblock = c.sponsorblock.clone();
                self.settings.split_chapters = c.chapters;
                self.settings.loudnorm = c.loudnorm;
                self.settings.strip_exif = c.strip_exif;
                self.settings.image_max_kb = c.max_kb.map(|n| n.to_string()).unwrap_or_default();
                self.settings.save()?;
                self.enqueue(c);
            } else {
                let old = serde_json::to_value(&self.settings)?;
                let mut v = old.clone();
                for (k, s) in &form.rows {
                    v[k] = if old[k].is_boolean() {
                        Value::Bool(s.parse()?)
                    } else if old[k].is_number() {
                        json!(s.parse::<usize>()?)
                    } else {
                        json!(s)
                    }
                }
                let settings: Settings = serde_json::from_value(v)?;
                if !(1..=4).contains(&settings.parallel) {
                    anyhow::bail!("parallel must be 1–4")
                };
                download::template(&settings.filename_template)?;
                settings.save()?;
                self.settings = settings;
                self.status = "Settings saved".into();
            }
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.form = None;
                self.focus = None;
            }
            Err(e) => self.status = format!("{e:#}"),
        }
        Ok(())
    }
    fn key(&mut self, k: KeyEvent) -> Result<bool> {
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            return Ok(true);
        }
        if self.browser.is_some() {
            return self.browser_key(k);
        }
        if let Some((index, values, selected)) = self.dropdown.clone() {
            match k.code {
                KeyCode::Esc => self.dropdown = None,
                KeyCode::Up | KeyCode::BackTab => {
                    self.dropdown = Some((
                        index,
                        values.clone(),
                        (selected + values.len() - 1) % values.len(),
                    ))
                }
                KeyCode::Down | KeyCode::Tab => {
                    self.dropdown = Some((index, values.clone(), (selected + 1) % values.len()))
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    self.set_form_value(index, values[selected].clone());
                    self.dropdown = None;
                }
                _ => (),
            }
            return Ok(false);
        }
        if let Some(selected) = self.theme_picker {
            match k.code {
                KeyCode::Esc => self.finish_theme(false),
                KeyCode::Enter | KeyCode::Char(' ') => self.finish_theme(true),
                KeyCode::Up | KeyCode::Left | KeyCode::BackTab | KeyCode::Char('k') => {
                    self.preview_theme((selected + THEMES.len() - 1) % THEMES.len())
                }
                KeyCode::Down | KeyCode::Right | KeyCode::Tab | KeyCode::Char('j') => {
                    self.preview_theme((selected + 1) % THEMES.len())
                }
                KeyCode::Home => self.preview_theme(0),
                KeyCode::End => self.preview_theme(THEMES.len() - 1),
                _ => (),
            }
            return Ok(false);
        }
        if self.choosing {
            if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('s') {
                self.download_selection();
                return Ok(false);
            }
            match k.code {
                KeyCode::Esc => self.cancel_selection(),
                KeyCode::Char('a') => self.select_all_playlist(true),
                KeyCode::Char('n') => self.select_all_playlist(false),
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    self.playlist_first.push(c);
                    self.playlist_typing = true;
                }
                KeyCode::Backspace if self.playlist_typing => {
                    self.playlist_first.pop();
                }
                KeyCode::Tab => self.playlist_typing = !self.playlist_typing,
                KeyCode::Enter if self.playlist_typing => self.download_selection(),
                KeyCode::Up => self.selected = self.selected.saturating_sub(1),
                KeyCode::Down => {
                    self.playlist_typing = false;
                    self.selected = (self.selected + 1).min(self.entries.len().saturating_sub(1));
                }
                KeyCode::Char(' ') | KeyCode::Enter => {
                    if let Some(e) = self.entries.get_mut(self.selected) {
                        e.selected = !e.selected;
                    }
                }
                _ => (),
            }
            return Ok(false);
        }
        if self.form.is_none()
            && k.modifiers.contains(KeyModifiers::CONTROL)
            && k.code == KeyCode::Char('t')
        {
            self.page = if self.page == Page::Files {
                Page::Download
            } else {
                Page::Files
            };
            self.selected = 0;
            return Ok(false);
        }
        if matches!(self.page, Page::History | Page::Sites) && k.code == KeyCode::Esc {
            self.page = self.return_page;
            self.input.clear();
            self.editing = false;
            return Ok(false);
        }
        if self.page == Page::History && !self.editing {
            match k.code {
                KeyCode::Char('s') => {
                    if let Some(v) = self.filtered_history().get(self.selected) {
                        open(Path::new(v["path"].as_str().unwrap_or("")), true);
                    }
                    return Ok(false);
                }
                KeyCode::Char('g') => {
                    if let Some(v) = self.filtered_history().get(self.selected) {
                        let url = v["url"].as_str().unwrap_or("").to_owned();
                        self.page = self.return_page;
                        self.load_input(url);
                    }
                    return Ok(false);
                }
                KeyCode::Char('c') => {
                    if let Some(v) = self.filtered_history().get(self.selected) {
                        clipboard_write(v["url"].as_str().unwrap_or(""));
                    }
                    return Ok(false);
                }
                KeyCode::Char('x') => {
                    if let Some(v) = self.filtered_history().get(self.selected) {
                        state::forget(v["id"].as_str().unwrap_or(""))?;
                        self.history = state::history();
                    }
                    return Ok(false);
                }
                _ => (),
            }
        }

        if self
            .form
            .as_ref()
            .is_some_and(|form| form.purpose == "settings")
            && k.modifiers.contains(KeyModifiers::CONTROL)
            && k.code == KeyCode::Char('b')
        {
            self.folder_pick = true;
            self.open_browser(std::env::current_dir()?)?;
            return Ok(false);
        }
        if self.form.is_some() {
            if k.code == KeyCode::Esc {
                self.form = None;
                self.focus = None;
                return Ok(false);
            }
            if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('s') {
                self.submit_form()?;
                return Ok(false);
            }
            if k.code == KeyCode::Tab || k.code == KeyCode::BackTab {
                let form = self.form.as_ref().unwrap();
                let mut targets = self
                    .visible_form_fields(form)
                    .into_iter()
                    .map(UiAction::Field)
                    .collect::<Vec<_>>();
                targets.extend([UiAction::Submit, UiAction::Close]);
                let current = self.focus.clone().unwrap_or(UiAction::Field(form.selected));
                let index = targets.iter().position(|a| *a == current).unwrap_or(0);
                let back = k.code == KeyCode::BackTab || k.modifiers.contains(KeyModifiers::SHIFT);
                let next = targets[if back {
                    (index + targets.len() - 1) % targets.len()
                } else {
                    (index + 1) % targets.len()
                }]
                .clone();
                self.focus = Some(next.clone());
                if let UiAction::Field(i) = next {
                    let form = self.form.as_mut().unwrap();
                    form.selected = i;
                    form.cursor = None;
                }
                return Ok(false);
            }
            if matches!(self.focus, Some(UiAction::Submit | UiAction::Close)) {
                if matches!(k.code, KeyCode::Enter | KeyCode::Char(' ')) {
                    self.activate(self.focus.clone().unwrap())?;
                } else if matches!(k.code, KeyCode::Left | KeyCode::Right) {
                    self.focus = Some(if self.focus == Some(UiAction::Submit) {
                        UiAction::Close
                    } else {
                        UiAction::Submit
                    });
                }
                return Ok(false);
            }
            let selected = self.form.as_ref().unwrap().selected;
            let key = self.form.as_ref().unwrap().rows[selected].0.clone();
            let options = self.form_choices(&key);
            if self.form.as_ref().unwrap().purpose == "settings"
                && !options.is_empty()
                && matches!(k.code, KeyCode::Enter | KeyCode::Char(' '))
            {
                let value = &self.form.as_ref().unwrap().rows[selected].1;
                let i = options.iter().position(|v| v == value).unwrap_or(0);
                self.dropdown = Some((selected, options, i));
                return Ok(false);
            }

            if options.is_empty() {
                let form = self.form.as_mut().unwrap();
                if crate::text_input::edit(&mut form.rows[selected].1, &mut form.cursor, k) {
                    return Ok(false);
                }
            }

            if matches!(k.code, KeyCode::Left | KeyCode::Right)
                || matches!(k.code, KeyCode::Up | KeyCode::Down)
                    && ["format", "quality"].contains(&key.as_str())
                || k.code == KeyCode::Enter && !options.is_empty()
                || k.code == KeyCode::Char(' ') && !options.is_empty()
            {
                if !options.is_empty() {
                    let current = &self.form.as_ref().unwrap().rows[selected].1;
                    let i = options.iter().position(|v| v == current).unwrap_or(0);
                    let backward = matches!(k.code, KeyCode::Left | KeyCode::Up);
                    let next = if backward {
                        (i + options.len() - 1) % options.len()
                    } else {
                        (i + 1) % options.len()
                    };
                    self.set_form_value(selected, options[next].clone());
                }
                return Ok(false);
            }
            let visible = self.visible_form_fields(self.form.as_ref().unwrap());
            let form = self.form.as_mut().unwrap();
            match k.code {
                KeyCode::Esc => self.form = None,
                KeyCode::Up | KeyCode::Down | KeyCode::Tab => {
                    let i = visible
                        .iter()
                        .position(|i| *i == form.selected)
                        .unwrap_or(0);
                    let back = k.code == KeyCode::Up || k.modifiers.contains(KeyModifiers::SHIFT);
                    form.cursor = None;
                    form.selected = visible[if back {
                        (i + visible.len() - 1) % visible.len()
                    } else {
                        (i + 1) % visible.len()
                    }];
                    self.focus = Some(UiAction::Field(form.selected));
                }
                _ => (),
            }
            return Ok(false);
        }
        if self.editing {
            if crate::text_input::edit(&mut self.input, &mut self.input_cursor, k) {
                self.selected = 0;
                if self.page == Page::Download {
                    let text = self.input.trim();
                    if !text.contains(char::is_whitespace)
                        && (text.starts_with("http://")
                            || text.starts_with("https://")
                            || text.contains('.'))
                        && self.info.as_ref().and_then(|i| i["webpage_url"].as_str()) != Some(text)
                    {
                        self.pending_url = Some((Instant::now(), self.input.clone()));
                    } else {
                        self.pending_url = None;
                    }
                }
                return Ok(false);
            }
            match k.code {
                KeyCode::Tab | KeyCode::BackTab => {
                    self.editing = false;
                    self.focus = if k.code == KeyCode::BackTab
                        || k.modifiers.contains(KeyModifiers::SHIFT)
                    {
                        self.focusable_buttons.last().cloned()
                    } else {
                        self.focusable_buttons.first().cloned()
                    };
                }
                KeyCode::Esc => {
                    self.input_cursor = None;
                }
                KeyCode::Down if self.item_count() > 0 => {
                    self.editing = false;
                    self.focus = Some(UiAction::Select(self.selected));
                }
                KeyCode::Enter => {
                    if matches!(self.page, Page::Download | Page::Files) {
                        self.load_input(self.input.clone());
                    } else {
                        self.editing = false;
                    }
                }
                _ => (),
            }
            return Ok(false);
        }
        if matches!(self.page, Page::Download | Page::Files)
            && matches!(k.code, KeyCode::Enter | KeyCode::Char(' '))
            && let Some(action @ (UiAction::Key(_) | UiAction::Retry | UiAction::CancelJob(_))) =
                self.focus.clone()
        {
            self.activate(action)?;
            return Ok(false);
        }
        match k.code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Esc | KeyCode::Char('/') => self.editing = true,
            KeyCode::Char('1') => {
                self.page = Page::Download;
                self.editing = true;
                self.selected = 0;
                self.input.clear()
            }
            KeyCode::Char('2') => {
                self.page = Page::Files;
                self.editing = true;
                self.selected = 0;
                self.input.clear()
            }
            KeyCode::Char('3') | KeyCode::Char('s') => self.settings_form(),
            KeyCode::Char('4') | KeyCode::Char('l') => {
                if !matches!(self.page, Page::History | Page::Sites) {
                    self.return_page = self.page;
                }
                self.page = Page::Sites;
                self.editing = true;
                self.input.clear();
                self.selected = 0;
                if self.sites.is_empty() {
                    self.loading_sites = true;
                    let tx = self.tx.clone();
                    thread::spawn(move || {
                        let _ =
                            tx.send(Message::Sites(download::sites().map_err(|e| e.to_string())));
                    });
                }
            }
            KeyCode::Char('h') => {
                if !matches!(self.page, Page::History | Page::Sites) {
                    self.return_page = self.page;
                }
                self.page = Page::History;
                self.editing = true;
                self.input.clear();
                self.selected = 0;
                self.history = state::history()
            }
            KeyCode::Char('t') => {
                self.theme_original = Some(self.settings.theme.clone());
                self.theme_scroll = 0;
                self.theme_picker = Some(
                    THEMES
                        .iter()
                        .position(|t| *t == self.settings.theme)
                        .unwrap_or(0),
                )
            }
            KeyCode::Char('p') => {
                self.paused = !self.paused;
                self.status = if self.paused {
                    "Queue paused"
                } else {
                    "Queue resumed"
                }
                .into()
            }
            KeyCode::Char('x') => {
                if let Some(j) = self.jobs.iter_mut().find(|j| !j.done) {
                    j.cancel.store(true, Ordering::Relaxed);
                    if !j.started {
                        j.done = true;
                        j.status = "Cancelled".into();
                    }
                }
            }
            KeyCode::Char('c') => self.options(),
            KeyCode::Char('d') => {
                if self.page == Page::History {
                    if let Some(v) = self.filtered_history().get(self.selected) {
                        let url = v["url"].as_str().unwrap_or("").to_owned();
                        if !url.is_empty() {
                            self.load_input(url);
                        }
                    }
                } else {
                    let mut c = Choice::saved(&self.settings);
                    c.kind = "video".into();
                    c.fmt = "mp4".into();
                    c.quality = "best".into();
                    self.enqueue(c)
                }
            }
            KeyCode::Char('a') => {
                let mut c = Choice::saved(&self.settings);
                c.kind = "audio".into();
                c.fmt = self.settings.last_audio_format.clone();
                c.quality = self.settings.last_audio_quality.clone();
                self.enqueue(c)
            }
            KeyCode::Char('e') => {
                if self.info.as_ref().is_some_and(|i| i["entries"].is_array()) {
                    self.choosing = true;
                    self.playlist_backup = self.entries.iter().map(|e| e.selected).collect();
                    self.playlist_first.clear();
                    self.playlist_typing = false;
                    self.selected = 0;
                }
            }
            KeyCode::Char(' ') => {
                if let Some(e) = self.entries.get_mut(self.selected) {
                    e.selected = !e.selected
                }
            }
            KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                self.focus = Some(UiAction::Select(self.selected));
            }
            KeyCode::Down => {
                self.selected = self
                    .selected
                    .saturating_add(1)
                    .min(self.item_count().saturating_sub(1));
                self.focus = Some(UiAction::Select(self.selected));
            }
            KeyCode::Char('r') => {
                if self.page == Page::Files && self.selected < self.files.len() {
                    self.files.remove(self.selected);
                    self.selected = self.selected.min(self.files.len().saturating_sub(1));
                } else if self.page == Page::History
                    && let Some(v) = self.filtered_history().get(self.selected)
                {
                    state::forget(v["id"].as_str().unwrap_or(""))?;
                    self.history = state::history();
                }
            }
            KeyCode::Char('o') => {
                if let Some(path) = &self.last_saved {
                    open(path, false);
                }
            }
            KeyCode::Enter => match self.page {
                Page::History => {
                    if let Some(v) = self.filtered_history().get(self.selected) {
                        open(Path::new(v["path"].as_str().unwrap_or("")), false);
                    }
                }
                Page::Files => self.options(),
                _ => (),
            },
            KeyCode::Char('v') => {
                if let Some(text) = clipboard_read() {
                    self.motion.pasted = Some(Instant::now());
                    self.load_input(text)
                }
            }
            KeyCode::Char('y') => {
                if let Some(v) = self.filtered_history().get(self.selected) {
                    clipboard_write(v["url"].as_str().unwrap_or(""));
                }
            }
            KeyCode::Char('f') => self.open_browser(std::env::current_dir()?)?,
            KeyCode::Char('u') => {
                let tx = self.tx.clone();
                self.status = "Updating yt-dlp…".into();
                self.updating = true;
                thread::spawn(move || {
                    let report: Reporter = Arc::new(|_| {});
                    let c = Arc::new(AtomicBool::new(false));
                    let _ = tx.send(Message::Update(
                        download::update(&c, &report).map_err(|e| e.to_string()),
                    ));
                });
            }
            KeyCode::Tab | KeyCode::BackTab => {
                let back = k.code == KeyCode::BackTab || k.modifiers.contains(KeyModifiers::SHIFT);
                let index = self
                    .focus
                    .as_ref()
                    .and_then(|a| self.focusable_buttons.iter().position(|b| a == b));
                let next = if back {
                    index.and_then(|i| i.checked_sub(1))
                } else {
                    Some(index.map(|i| i + 1).unwrap_or(0))
                };
                self.focus = next.and_then(|i| self.focusable_buttons.get(i)).cloned();
                self.editing = self.focus.is_none();
            }
            _ => (),
        }
        Ok(false)
    }
    fn filtered_history(&self) -> Vec<Value> {
        let q = self.input.to_lowercase();
        self.history
            .iter()
            .filter(|v| v.to_string().to_lowercase().contains(&q))
            .cloned()
            .collect()
    }
    fn item_count(&self) -> usize {
        match self.page {
            Page::Download => self.entries.len(),
            Page::Files => self.files.len(),
            Page::History => self.filtered_history().len(),
            Page::Sites => self
                .sites
                .iter()
                .filter(|s| s.to_lowercase().contains(&self.input.to_lowercase()))
                .count(),
        }
    }
    fn open_browser(&mut self, dir: PathBuf) -> Result<()> {
        let mut entries = fs::read_dir(&dir)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .collect::<Vec<_>>();
        entries.sort_by_key(|p| (!p.is_dir(), p.file_name().map(|s| s.to_ascii_lowercase())));
        if let Some(p) = dir.parent() {
            entries.insert(0, p.to_owned());
        }
        self.browser = Some(Browser {
            dir,
            entries,
            selected: 0,
        });
        Ok(())
    }
    fn browser_key(&mut self, k: KeyEvent) -> Result<bool> {
        let b = self.browser.as_mut().unwrap();
        match k.code {
            KeyCode::Char(' ') if self.folder_pick => {
                let value = b.dir.to_string_lossy().into_owned();
                if let Some(form) = &mut self.form
                    && let Some(row) = form.rows.iter_mut().find(|(key, _)| key == "download_dir")
                {
                    row.1 = value;
                }
                self.browser = None;
                self.folder_pick = false;
            }
            KeyCode::Esc => {
                self.browser = None;
                self.folder_pick = false;
            }
            KeyCode::Up => b.selected = b.selected.saturating_sub(1),
            KeyCode::Down => b.selected = (b.selected + 1).min(b.entries.len().saturating_sub(1)),
            KeyCode::Enter => {
                if let Some(p) = b.entries.get(b.selected).cloned() {
                    if p.is_dir() {
                        if let Err(e) = self.open_browser(p) {
                            self.status = e.to_string()
                        }
                    } else {
                        self.browser = None;
                        self.load_input(p.to_string_lossy().into());
                    }
                }
            }
            _ => (),
        }
        Ok(false)
    }
    fn draw(&mut self, f: &mut Frame) {
        self.draw_original(f);
        crate::terminal_colors::adapt(f.buffer_mut(), crate::terminal_colors::truecolor());
    }
}
#[path = "presentation.rs"]
mod presentation;

fn duration(s: f64) -> String {
    format!("{}:{:02}", s as u64 / 60, s as u64 % 60)
}
fn decode_file_url(s: &str) -> String {
    let s = s.strip_prefix("file://").unwrap_or(s);
    let mut bytes = Vec::new();
    let mut i = 0;
    while i < s.len() {
        if s.as_bytes()[i] == b'%'
            && i + 2 < s.len()
            && let Ok(v) = s
                .get(i + 1..i + 3)
                .and_then(|v| u8::from_str_radix(v, 16).ok())
                .ok_or(())
        {
            bytes.push(v);
            i += 3;
            continue;
        }
        bytes.push(s.as_bytes()[i]);
        i += 1;
    }
    String::from_utf8_lossy(&bytes).into_owned()
}
fn clipboard_read() -> Option<String> {
    let mut c = if cfg!(target_os = "macos") {
        Command::new("pbpaste")
    } else if cfg!(windows) {
        let mut c = Command::new("powershell");
        c.args(["-NoProfile", "-Command", "Get-Clipboard"]);
        c
    } else {
        let mut c = Command::new("xclip");
        c.args(["-selection", "clipboard", "-o"]);
        c
    };
    c.output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
}
fn clipboard_write(text: &str) {
    use std::io::Write;
    let mut c = if cfg!(target_os = "macos") {
        Command::new("pbcopy")
    } else if cfg!(windows) {
        Command::new("clip")
    } else {
        let mut c = Command::new("xclip");
        c.args(["-selection", "clipboard"]);
        c
    };
    if let Ok(mut child) = c.stdin(std::process::Stdio::piped()).spawn() {
        if let Some(mut input) = child.stdin.take() {
            let _ = input.write_all(text.as_bytes());
        }
        let _ = child.wait();
    }
}
fn open(path: &Path, reveal: bool) {
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        if reveal {
            c.arg("-R");
        }
        c
    } else if cfg!(windows) {
        Command::new("explorer")
    } else {
        Command::new("xdg-open")
    };
    let _ = cmd
        .arg(path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}
fn notify(text: &str) {
    if cfg!(target_os = "macos") {
        let _ = Command::new("osascript")
            .args([
                "-e",
                &format!("display notification {text:?} with title \"gingertea\""),
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    } else if cfg!(target_os = "linux") {
        let _ = Command::new("notify-send")
            .args(["gingertea", text])
            .spawn();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drop_urls() {
        assert_eq!(
            decode_file_url("file:///tmp/My%20File.mp4"),
            "/tmp/My File.mp4"
        );
    }
}
