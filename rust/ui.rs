use crate::{
    download,
    media::{self, Cancel, Choice, FileInfo, Reporter},
    motion::{self, Motion},
    state::{self, Settings},
};
use anyhow::Result;
use crossterm::{
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers,
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
    "masala-chai",
    "matcha",
    "earl-grey",
    "hibiscus",
    "tea-house",
    "tea-house-dark",
    "gingertea",
    "gingertea-light",
    "dusk",
    "tide",
];
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
}
struct Browser {
    dir: PathBuf,
    entries: Vec<PathBuf>,
    selected: usize,
}
struct App {
    settings: Settings,
    page: Page,
    input: String,
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
}
struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), DisableBracketedPaste, LeaveAlternateScreen);
    }
}
pub fn run(settings: Settings, initial: String, cancel: Cancel) -> Result<()> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, EnableBracketedPaste)?;
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
    };
    if !initial.is_empty() {
        app.load_input(initial)
    }
    let mut was_animating = false;
    loop {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        app.messages();
        app.schedule();
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
                Event::Key(k) if k.kind == KeyEventKind::Press => {
                    if app.key(k)? {
                        break;
                    }
                    app.dirty = true
                }
                Event::Paste(s) => {
                    app.motion.pasted = Some(Instant::now());
                    if app.form.is_some() {
                        if let Some(form) = &mut app.form {
                            form.rows[form.selected].1.push_str(&s);
                        }
                    } else {
                        app.load_input(s);
                    }
                    app.dirty = true
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
    fn busy(&self) -> bool {
        self.loading_video.is_some()
            || self.loading_files > 0
            || self.loading_sites
            || self.updating
            || self.jobs.iter().any(|j| j.started && !j.done)
    }
    fn load_input(&mut self, text: String) {
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
            self.editing = false;
        } else {
            self.page = Page::Download;
            self.input = trimmed.into();
            self.thumb = None;
            self.info = None;
            self.entries.clear();
            self.generation += 1;
            let generation = self.generation;
            self.loading_video = Some(generation);
            let tx = self.tx.clone();
            let s = self.settings.clone();
            let input = self.input.clone();
            self.status = "Getting video details…".into();
            self.editing = false;
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
                        Err(e) => self.status = e,
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
        if self.page == Page::Files
            && let Some(f) = self.files.get(self.selected)
        {
            c.kind = f.media.clone();
            c.fmt = match f.media.as_str() {
                "audio" => self.settings.last_audio_format.clone(),
                "image" => self.settings.last_image_format.clone(),
                _ => self.settings.last_video_format.clone(),
            };
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
        self.form = Some(Form {
            title: "Convert options · ↑↓ field · ←→ choices · Ctrl+U clear · Ctrl+S Run".into(),
            rows,
            selected: 0,
            purpose: "convert".into(),
        });
    }
    fn settings_form(&mut self) {
        let value = serde_json::to_value(&self.settings).unwrap();
        let rows = value
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    if v.is_string() {
                        v.as_str().unwrap().into()
                    } else {
                        v.to_string()
                    },
                )
            })
            .collect();
        self.form = Some(Form {
            title: "Settings · ↑↓ field · Ctrl+U clear · Ctrl+S Save".into(),
            rows,
            selected: 0,
            purpose: "settings".into(),
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
                let mut v = json!({});
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
            Ok(()) => self.form = None,
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
        if self.form.is_some() {
            if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('s') {
                self.submit_form()?;
                return Ok(false);
            }
            let form = self.form.as_mut().unwrap();
            match k.code {
                KeyCode::Esc => self.form = None,
                KeyCode::Up => form.selected = form.selected.saturating_sub(1),
                KeyCode::Down | KeyCode::Tab => {
                    form.selected = (form.selected + 1) % form.rows.len()
                }
                KeyCode::Char('u') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                    form.rows[form.selected].1.clear()
                }
                KeyCode::Backspace => {
                    form.rows[form.selected].1.pop();
                }
                KeyCode::Left | KeyCode::Right => {
                    let (key, current) = &mut form.rows[form.selected];
                    let choices: Vec<String> = match key.as_str() {
                        "kind" => ["video", "audio", "image", "frames"]
                            .map(String::from)
                            .to_vec(),
                        "format" => media::formats(
                            &form
                                .rows
                                .iter()
                                .find(|(k, _)| k == "kind")
                                .map(|(_, v)| v.clone())
                                .unwrap_or("video".into()),
                        ),
                        "quality" => [
                            "best", "2160", "1440", "1080", "720", "480", "360", "320", "192",
                            "128", "50", "10", "1", "0.1",
                        ]
                        .map(String::from)
                        .to_vec(),
                        "subtitles" | "subtitle_mode" => {
                            ["off", "embed", "srt"].map(String::from).to_vec()
                        }
                        "sponsorblock" => ["off", "cut", "chapters"].map(String::from).to_vec(),
                        "theme" => THEMES.iter().map(|s| s.to_string()).collect(),
                        _ if current == "true" || current == "false" => {
                            vec!["false".into(), "true".into()]
                        }
                        _ => Vec::new(),
                    };
                    if !choices.is_empty() {
                        let (key, current) = &mut form.rows[form.selected];
                        let changed_kind = key == "kind";
                        let i = choices.iter().position(|v| v == current).unwrap_or(0);
                        let next = if k.code == KeyCode::Left {
                            (i + choices.len() - 1) % choices.len()
                        } else {
                            (i + 1) % choices.len()
                        };
                        *current = choices[next].clone();
                        if changed_kind {
                            let kind = current.clone();
                            for (key, value) in &mut form.rows {
                                if key == "format" {
                                    *value = match kind.as_str() {
                                        "audio" => "mp3",
                                        "image" => "jpg",
                                        "frames" => "png",
                                        _ => "mp4",
                                    }
                                    .into();
                                } else if key == "quality" {
                                    *value = if kind == "frames" { "1" } else { "best" }.into();
                                }
                            }
                        }
                    }
                }
                KeyCode::Char(c) => form.rows[form.selected].1.push(c),
                _ => (),
            }
            return Ok(false);
        }
        if self.editing {
            match k.code {
                KeyCode::Esc | KeyCode::Tab => self.editing = false,
                KeyCode::Enter => {
                    let input = self.input.clone();
                    if self.page == Page::Download || self.page == Page::Files {
                        self.load_input(input)
                    } else {
                        self.editing = false;
                    }
                }
                KeyCode::Backspace => {
                    self.input.pop();
                    self.selected = 0;
                }
                KeyCode::Char('u') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.input.clear()
                }
                KeyCode::Char(c) => {
                    self.input.push(c);
                    self.selected = 0;
                }
                _ => (),
            }
            return Ok(false);
        }
        match k.code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Esc | KeyCode::Char('/') => self.editing = true,
            KeyCode::Char('1') => {
                self.page = Page::Download;
                self.selected = 0;
                self.input.clear()
            }
            KeyCode::Char('2') => {
                self.page = Page::Files;
                self.selected = 0;
                self.input.clear()
            }
            KeyCode::Char('3') | KeyCode::Char('s') => self.settings_form(),
            KeyCode::Char('4') | KeyCode::Char('l') => {
                self.page = Page::Sites;
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
                self.page = Page::History;
                self.input.clear();
                self.selected = 0;
                self.history = state::history()
            }
            KeyCode::Char('t') => {
                let i = THEMES
                    .iter()
                    .position(|t| *t == self.settings.theme)
                    .unwrap_or(0);
                self.settings.theme = THEMES[(i + 1) % THEMES.len()].into();
                self.settings.save()?;
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
                let any = self.entries.iter().any(|e| !e.selected);
                for e in &mut self.entries {
                    e.selected = any
                }
            }
            KeyCode::Char(' ') => {
                if let Some(e) = self.entries.get_mut(self.selected) {
                    e.selected = !e.selected
                }
            }
            KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Down => {
                self.selected = self
                    .selected
                    .saturating_add(1)
                    .min(self.item_count().saturating_sub(1))
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
            KeyCode::Tab => {
                self.page = if self.page == Page::Download {
                    Page::Files
                } else {
                    Page::Download
                };
                self.selected = 0
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
            KeyCode::Esc => self.browser = None,
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
        let area = f.area();
        let (bg, fg, accent) = palette(&self.settings.theme);
        let now = Instant::now();
        let highlight = motion::mix(accent, fg, 0.85);
        let mut header = vec![Span::raw(" ")];
        header.extend(self.motion.logo(now).into_iter().map(|(c, bright)| {
            Span::styled(
                c.to_string(),
                Style::default()
                    .fg(if bright { highlight } else { accent })
                    .bold(),
            )
        }));
        header.push(Span::raw(
            "  1 Download   2 Convert files   h History   l Sites   s Settings",
        ));
        f.render_widget(Block::default().style(Style::default().bg(bg).fg(fg)), area);
        let layout = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(4),
            Constraint::Length(area.height.saturating_div(4).clamp(3, 9)),
            Constraint::Length(3),
            Constraint::Length(2),
        ])
        .split(area);
        f.render_widget(
            Paragraph::new(Line::from(header))
                .block(Block::bordered())
                .style(Style::default().fg(fg)),
            layout[0],
        );
        let input_title = match self.page {
            Page::Download => "Link or search",
            Page::Files => "Drop file paths · f Browse",
            Page::History => "Search history",
            Page::Sites => "Search supported sites",
        };
        f.render_widget(
            Paragraph::new(self.input.as_str()).block(
                Block::bordered()
                    .title(format!(
                        "{input_title}{}",
                        if self.editing { " · typing" } else { "" }
                    ))
                    .title_bottom(if self.loading_video.is_some() || self.loading_files > 0 {
                        format!(" {} Reading… ", self.motion.spinner(now))
                    } else {
                        String::new()
                    })
                    .border_style(Style::default().fg(motion::mix(
                        if self.editing {
                            accent
                        } else {
                            Color::Rgb(75, 75, 75)
                        },
                        highlight,
                        self.motion.input_strength(now),
                    ))),
            ),
            layout[1],
        );
        if self.editing {
            let width = layout[1].width.saturating_sub(3) as usize;
            f.set_cursor_position((
                layout[1].x + 1 + self.input.chars().count().min(width) as u16,
                layout[1].y + 1,
            ));
        }
        let rows: Vec<ListItem> = match self.page {
            Page::Download => self
                .entries
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    ListItem::new(format!(
                        "{} {:>3}  {}\n        {}",
                        if e.selected { "✓" } else { " " },
                        i + 1,
                        e.title,
                        e.detail
                    ))
                })
                .collect(),
            Page::Files => self
                .files
                .iter()
                .map(|v| {
                    ListItem::new(format!(
                        "{}  · {} · {}",
                        v.path.display(),
                        v.media,
                        duration(v.duration)
                    ))
                })
                .collect(),
            Page::History => self
                .filtered_history()
                .iter()
                .map(|v| {
                    ListItem::new(format!(
                        "{}\n{}",
                        v["title"].as_str().unwrap_or(""),
                        v["path"].as_str().unwrap_or("")
                    ))
                })
                .collect(),
            Page::Sites => self
                .sites
                .iter()
                .filter(|s| s.to_lowercase().contains(&self.input.to_lowercase()))
                .map(|s| ListItem::new(s.as_str()))
                .collect(),
        };
        let title = match self.page {
            Page::Download => self
                .info
                .as_ref()
                .and_then(|v| v["title"].as_str())
                .unwrap_or("Videos"),
            Page::Files => "Convert files · c Options · r Remove · f Browse",
            Page::History => "History · Enter Open · d Download again · y Copy URL · r Forget",
            Page::Sites => "Supported sites",
        };
        let mut state = ListState::default().with_selected(Some(self.selected));
        let list_area =
            if self.page == Page::Download && self.thumb.is_some() && layout[2].width >= 90 {
                let chunks = Layout::horizontal([Constraint::Length(38), Constraint::Min(20)])
                    .split(layout[2]);
                if let Some(t) = &self.thumb {
                    let lines = (0..18)
                        .step_by(2)
                        .map(|y| {
                            Line::from(
                                (0..36)
                                    .map(|x| {
                                        let top = t[y][x];
                                        let bottom = t[y + 1][x];
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
                    f.render_widget(Paragraph::new(lines).block(Block::bordered()), chunks[0]);
                }
                chunks[1]
            } else {
                layout[2]
            };
        if rows.is_empty() {
            f.render_widget(
                Paragraph::new(match self.page {
                    Page::Download => "No downloads yet. Paste a link, then choose Download.",
                    Page::Files => "Drop files here, or press f to browse",
                    Page::History => "No matching history",
                    Page::Sites => "Loading supported sites…",
                })
                .block(Block::bordered().title(title).border_style(
                    Style::default().fg(motion::mix(
                        Color::Rgb(75, 75, 75),
                        accent,
                        self.motion.ready_strength(now),
                    )),
                )),
                list_area,
            );
        } else {
            f.render_stateful_widget(
                List::new(rows)
                    .block(
                        Block::bordered()
                            .title(title)
                            .border_style(Style::default().fg(motion::mix(
                                Color::Rgb(75, 75, 75),
                                accent,
                                self.motion.ready_strength(now),
                            ))),
                    )
                    .highlight_style(Style::default().bg(accent).fg(bg))
                    .highlight_symbol("› "),
                list_area,
                &mut state,
            );
        }
        let jobs: Vec<_> = self
            .jobs
            .iter()
            .rev()
            .take(20)
            .map(|j| {
                let active = j.started && !j.done;
                let icon = if j.done {
                    if j.error { "✕" } else { "✓" }
                } else if active {
                    self.motion.spinner(now)
                } else {
                    "·"
                };
                let color = if j.done {
                    if j.error {
                        Color::Rgb(225, 120, 120)
                    } else {
                        Color::Rgb(135, 180, 135)
                    }
                } else {
                    accent
                };
                let mut row = vec![
                    Span::styled(format!("{icon} "), Style::default().fg(color)),
                    Span::raw(format!("{} · ", j.title)),
                ];
                if active {
                    row.push(Span::styled(
                        format!(
                            "{} ",
                            motion::progress_bar(
                                j.fraction,
                                self.motion.phase(now),
                                self.motion.enabled
                            )
                        ),
                        Style::default().fg(accent),
                    ));
                }
                row.push(Span::raw(j.status.clone()));
                ListItem::new(Line::from(row))
            })
            .collect();
        f.render_widget(
            List::new(jobs).block(
                Block::bordered()
                    .border_style(Style::default().fg(motion::mix(
                        Color::Rgb(75, 75, 75),
                        if self.motion.finished.is_some_and(|(_, success)| success) {
                            Color::Rgb(135, 180, 135)
                        } else {
                            Color::Rgb(225, 120, 120)
                        },
                        self.motion.finished_strength(now),
                    )))
                    .title(if self.paused {
                        "Queue · paused"
                    } else {
                        "Queue"
                    }),
            ),
            layout[3],
        );
        f.render_widget(
            Paragraph::new(if self.loading_sites || self.updating {
                format!("{} {}", self.motion.spinner(now), self.status)
            } else {
                self.status.clone()
            })
            .wrap(Wrap { trim: true })
            .block(Block::bordered().border_style(Style::default().fg(accent))),
            layout[4],
        );
        f.render_widget(Paragraph::new(" d Download  a Audio  c Options  p Pause  x Cancel  t Theme  o Open  u Update\n Tab Switch/focus  Esc Type  v Clipboard  q Quit").style(Style::default().fg(accent)),layout[5]);
        if let Some(form) = &self.form {
            let popup = center(area, 90, 85);
            f.render_widget(Clear, popup);
            let rows = form
                .rows
                .iter()
                .map(|(k, v)| ListItem::new(format!("{k:22} {v}")))
                .collect::<Vec<_>>();
            let mut state = ListState::default().with_selected(Some(form.selected));
            f.render_stateful_widget(
                List::new(rows)
                    .block(
                        Block::bordered()
                            .title(form.title.as_str())
                            .style(Style::default().bg(bg).fg(fg)),
                    )
                    .highlight_style(Style::default().bg(accent).fg(bg)),
                popup,
                &mut state,
            );
        }
        if let Some(browser) = &self.browser {
            let popup = center(area, 85, 85);
            f.render_widget(Clear, popup);
            let rows = browser
                .entries
                .iter()
                .map(|p| {
                    ListItem::new(format!(
                        "{} {}",
                        if p.is_dir() { "▸" } else { " " },
                        if browser.dir.parent() == Some(p.as_path()) {
                            "..".into()
                        } else {
                            p.file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned()
                        }
                    ))
                })
                .collect::<Vec<_>>();
            let mut state = ListState::default().with_selected(Some(browser.selected));
            f.render_stateful_widget(
                List::new(rows)
                    .block(
                        Block::bordered()
                            .title(format!(
                                "{} · Enter Open · Esc Close",
                                browser.dir.display()
                            ))
                            .style(Style::default().bg(bg).fg(fg)),
                    )
                    .highlight_style(Style::default().bg(accent).fg(bg)),
                popup,
                &mut state,
            );
        }
    }
}
fn center(area: Rect, w: u16, h: u16) -> Rect {
    let width = area.width * w / 100;
    let height = area.height * h / 100;
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}
fn duration(s: f64) -> String {
    format!("{}:{:02}", s as u64 / 60, s as u64 % 60)
}
fn palette(name: &str) -> (Color, Color, Color) {
    match name {
        "matcha" => (
            Color::Rgb(28, 38, 33),
            Color::Rgb(215, 223, 215),
            Color::Rgb(175, 215, 135),
        ),
        "earl-grey" => (
            Color::Rgb(28, 28, 38),
            Color::Rgb(215, 215, 223),
            Color::Rgb(175, 175, 215),
        ),
        "hibiscus" => (
            Color::Rgb(38, 28, 38),
            Color::Rgb(228, 215, 223),
            Color::Rgb(215, 135, 175),
        ),
        "tea-house" | "gingertea-light" => (
            Color::Rgb(238, 238, 238),
            Color::Rgb(38, 38, 38),
            Color::Rgb(95, 95, 0),
        ),
        "tea-house-dark" => (
            Color::Rgb(28, 28, 23),
            Color::Rgb(228, 228, 215),
            Color::Rgb(175, 175, 95),
        ),
        "dusk" => (
            Color::Rgb(22, 19, 31),
            Color::Rgb(232, 228, 240),
            Color::Rgb(175, 135, 255),
        ),
        "tide" => (
            Color::Rgb(14, 22, 32),
            Color::Rgb(225, 233, 242),
            Color::Rgb(95, 175, 255),
        ),
        "gingertea" => (
            Color::Rgb(18, 18, 18),
            Color::Rgb(228, 228, 228),
            Color::Rgb(255, 135, 0),
        ),
        _ => (
            Color::Rgb(18, 18, 18),
            Color::Rgb(228, 228, 228),
            Color::Rgb(215, 175, 95),
        ),
    }
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
    #[test]
    fn small_layout() {
        let r = center(Rect::new(0, 0, 20, 10), 90, 85);
        assert!(r.right() <= 20 && r.bottom() <= 10);
    }
}
