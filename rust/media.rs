use crate::state;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
pub type Cancel = Arc<AtomicBool>;
pub type Reporter = Arc<dyn Fn(String) + Send + Sync>;
pub fn cancelled(c: &Cancel) -> Result<()> {
    if c.load(Ordering::Relaxed) {
        bail!("Cancelled")
    }
    Ok(())
}
pub fn tables() -> &'static Value {
    static T: OnceLock<Value> = OnceLock::new();
    T.get_or_init(|| serde_json::from_str(include_str!("formats.json")).unwrap())
}
pub fn formats(kind: &str) -> Vec<String> {
    tables()[if kind == "frames" { "image" } else { kind }]
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Choice {
    pub kind: String,
    pub fmt: String,
    pub quality: String,
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub subtitles: String,
    pub languages: String,
    pub sponsorblock: String,
    pub chapters: bool,
    pub loudnorm: bool,
    pub max_kb: Option<u64>,
    pub strip_exif: bool,
}
impl Default for Choice {
    fn default() -> Self {
        Self {
            kind: "video".into(),
            fmt: "mp4".into(),
            quality: "best".into(),
            start: None,
            end: None,
            subtitles: "off".into(),
            languages: String::new(),
            sponsorblock: "off".into(),
            chapters: false,
            loudnorm: false,
            max_kb: None,
            strip_exif: false,
        }
    }
}
impl Choice {
    pub fn saved(s: &state::Settings) -> Self {
        Self {
            kind: s.last_kind.clone(),
            fmt: match s.last_kind.as_str() {
                "audio" => s.last_audio_format.clone(),
                "image" => s.last_image_format.clone(),
                "frames" => s.last_frames_format.clone(),
                _ => s.last_video_format.clone(),
            },
            quality: match s.last_kind.as_str() {
                "audio" => s.last_audio_quality.clone(),
                "image" => s.last_image_quality.clone(),
                "frames" => s.last_frames_quality.clone(),
                _ => s.last_video_quality.clone(),
            },
            subtitles: s.subtitle_mode.clone(),
            languages: s.subtitle_langs.clone(),
            sponsorblock: s.sponsorblock.clone(),
            chapters: s.split_chapters,
            loudnorm: s.loudnorm,
            max_kb: s.image_max_kb.parse().ok(),
            strip_exif: s.strip_exif,
            ..Self::default()
        }
    }
    pub fn validate(&self) -> Result<()> {
        if !formats(&self.kind).contains(&self.fmt) {
            bail!("Unsupported {} format: {}", self.kind, self.fmt)
        }
        if self.start.is_some_and(|v| !v.is_finite() || v < 0.)
            || self
                .end
                .is_some_and(|v| !v.is_finite() || v <= self.start.unwrap_or(0.))
        {
            bail!("End must be after start")
        }
        if self.quality != "best" {
            let n = self.quality.parse::<f64>().context("Invalid quality")?;
            if !n.is_finite() || n <= 0. {
                bail!("Quality must be positive")
            }
        }
        Ok(())
    }
}
pub fn executable(name: &str) -> PathBuf {
    let key = format!("GINGERTEA_{}", name.to_uppercase().replace('-', "_"));
    if let Ok(p) = std::env::var(key) {
        return p.into();
    }
    let bundled = state::data_dir().join("tools").join(if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.into()
    });
    if bundled.exists() {
        return bundled;
    }
    if name == "ffmpeg" || name == "ffprobe" {
        let old = state::data_dir()
            .join("ffmpeg")
            .join(platform_key())
            .join(if cfg!(windows) {
                format!("{name}.exe")
            } else {
                name.into()
            });
        if old.exists() {
            return old;
        }
    }
    // PyPI installs yt-dlp and Deno beside the native executable. uv/pipx expose
    // gingertea through a symlink without adding that environment to PATH.
    if let Ok(app) = std::env::current_exe()
        && let Some(bin) = app.parent()
    {
        let sibling = bin.join(if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.into()
        });
        if sibling.is_file() {
            return sibling;
        }
    }
    for bin in managed_tool_bins() {
        let tool = bin.join(if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.into()
        });
        if tool.is_file() {
            return tool;
        }
    }
    name.into()
}
fn managed_tool_bins() -> &'static Vec<PathBuf> {
    static BINS: OnceLock<Vec<PathBuf>> = OnceLock::new();
    BINS.get_or_init(|| {
        // Windows tool managers copy the executable into their public bin
        // directory, so current_exe has no dependency executables beside it.
        let bin = if cfg!(windows) { "Scripts" } else { "bin" };
        let mut roots = Vec::new();
        if let Some(root) = std::env::var_os("UV_TOOL_DIR") {
            roots.push(PathBuf::from(root));
        }
        if let Some(root) = std::env::var_os("PIPX_HOME") {
            roots.push(PathBuf::from(root).join("venvs"));
        }
        for (manager, args) in [
            ("uv", vec!["tool", "dir"]),
            ("pipx", vec!["environment", "--value", "PIPX_LOCAL_VENVS"]),
        ] {
            let mut command = Command::new(manager);
            command.args(args);
            if let Ok(root) = capture(command)
                && !root.trim().is_empty()
            {
                roots.push(PathBuf::from(root.trim()));
            }
        }
        roots
            .into_iter()
            .map(|root| root.join("gingertea").join(bin))
            .collect()
    })
}
pub fn platform_key() -> String {
    let os = if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(windows) {
        "win32"
    } else {
        "linux"
    };
    if cfg!(target_arch = "aarch64") && os != "win32" {
        format!("{os}_arm64")
    } else {
        os.into()
    }
}
pub fn command(name: &str) -> Command {
    Command::new(executable(name))
}
static WORKERS: std::sync::Mutex<Vec<u32>> = std::sync::Mutex::new(Vec::new());
struct WorkerRegistration(u32);
impl WorkerRegistration {
    fn new(id: u32) -> Self {
        WORKERS.lock().unwrap().push(id);
        Self(id)
    }
}
impl Drop for WorkerRegistration {
    fn drop(&mut self) {
        WORKERS.lock().unwrap().retain(|id| *id != self.0);
    }
}
pub fn shutdown_workers() {
    for id in WORKERS.lock().unwrap().iter() {
        #[cfg(unix)]
        unsafe {
            libc::kill(-(*id as i32), libc::SIGKILL);
        }
        #[cfg(windows)]
        {
            let _ = Command::new("taskkill")
                .args(["/T", "/F", "/PID", &id.to_string()])
                .output();
        }
    }
}
fn configure_process(_cmd: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        _cmd.process_group(0);
    }
}
fn kill_tree(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        // Media workers own their process group; include FFmpeg launched by yt-dlp.
        unsafe {
            libc::kill(-(child.id() as i32), libc::SIGKILL);
        }
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &child.id().to_string()])
            .output();
    }
    let _ = child.kill();
}
pub fn capture(cmd: Command) -> Result<String> {
    Ok(String::from_utf8(capture_bytes(cmd)?)?)
}
fn capture_bytes(mut cmd: Command) -> Result<Vec<u8>> {
    configure_process(&mut cmd);
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Could not start media tool; run gingertea setup")?;
    let _registration = WorkerRegistration::new(child.id());
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let err = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    let mut timed_out = false;
    let status = loop {
        if std::time::Instant::now() > deadline {
            timed_out = true;
            kill_tree(&mut child);
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        thread::sleep(Duration::from_millis(5));
    };
    let stdout = out
        .join()
        .map_err(|_| anyhow::anyhow!("Media output reader failed"))??;
    let stderr = err
        .join()
        .map_err(|_| anyhow::anyhow!("Media error reader failed"))??;
    if timed_out {
        bail!("Media probe timed out after 30 seconds")
    }
    if !status.success() {
        bail!("{}", String::from_utf8_lossy(&stderr).trim())
    }
    Ok(stdout)
}
pub fn run(mut cmd: Command, cancel: &Cancel, report: &Reporter) -> Result<()> {
    cancelled(cancel)?;
    configure_process(&mut cmd);
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Could not start media tool; run gingertea setup")?;
    let _registration = WorkerRegistration::new(child.id());
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let r = report.clone();
    let out = thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            r(line)
        }
    });
    let r = report.clone();
    let err = thread::spawn(move || {
        let mut tail = String::new();
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            r(line.clone());
            tail.push_str(&line);
            tail.push('\n');
            if tail.len() > 16384 {
                tail = tail
                    .chars()
                    .rev()
                    .take(8192)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect()
            }
        }
        tail
    });
    let status = loop {
        if cancel.load(Ordering::Relaxed) {
            kill_tree(&mut child);
        }
        if let Some(s) = child.try_wait()? {
            break s;
        }
        thread::sleep(Duration::from_millis(30));
    };
    let _ = out.join();
    let error = err.join().unwrap_or_default();
    cancelled(cancel)?;
    if !status.success() {
        bail!("{}", error.trim())
    }
    Ok(())
}
#[derive(Clone, Debug)]
pub struct FileInfo {
    pub path: PathBuf,
    pub raw: Value,
    pub media: String,
    pub video: Option<Value>,
    pub audio: Option<Value>,
    pub duration: f64,
}
pub fn probe(path: &Path) -> Result<FileInfo> {
    let ext = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if ["heic", "heif", "hif", "jxl"].contains(&ext.as_str()) {
        let work = tempfile::tempdir()?;
        let decoded = work.path().join("image.png");
        decode_image(path, &decoded)?;
        let mut info = probe(&decoded)?;
        info.path = path.into();
        info.media = "image".into();
        return Ok(info);
    }
    let mut c = command("ffprobe");
    c.args([
        "-v",
        "error",
        "-show_format",
        "-show_streams",
        "-show_chapters",
        "-of",
        "json",
    ])
    .arg(path);
    let raw: Value = serde_json::from_str(&capture(c)?)?;
    parse_probe(path, raw)
}
pub fn parse_probe(path: &Path, raw: Value) -> Result<FileInfo> {
    let streams = raw["streams"].as_array().context("No media streams")?;
    let video = streams
        .iter()
        .find(|s| {
            s["codec_type"] == "video"
                && s["disposition"]["attached_pic"].as_u64().unwrap_or(0) == 0
        })
        .cloned();
    let audio = streams.iter().find(|s| s["codec_type"] == "audio").cloned();
    let duration = number(&raw["format"]["duration"]);
    let name = raw["format"]["format_name"].as_str().unwrap_or("");
    let ext = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let media = if video.is_some() {
        if name.contains("_pipe")
            || name == "image2"
            || ["heic", "heif", "avif", "jxl"].contains(&ext.as_str())
            || name == "gif" && duration == 0.
        {
            "image"
        } else {
            "video"
        }
    } else if audio.is_some() {
        "audio"
    } else {
        bail!("File has no audio, video or image")
    };
    Ok(FileInfo {
        path: path.into(),
        raw,
        media: media.into(),
        video,
        audio,
        duration,
    })
}
pub fn number(v: &Value) -> f64 {
    v.as_f64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(0.)
}
fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|s| s.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}
fn allows(v: &Value, codec: &str) -> bool {
    v.is_null() || v.as_array().is_some_and(|a| a.iter().any(|v| v == codec))
}
fn encoder(args: &[String]) -> &str {
    args.windows(2)
        .find(|a| a[0] == "-c:v" || a[0] == "-c:a")
        .map(|a| a[1].as_str())
        .unwrap_or("")
}
pub fn encoders() -> &'static String {
    static E: OnceLock<String> = OnceLock::new();
    E.get_or_init(|| {
        let mut c = command("ffmpeg");
        c.args(["-hide_banner", "-encoders"]);
        capture(c).unwrap_or_default()
    })
}
fn has_encoder(name: &str) -> bool {
    encoders()
        .lines()
        .any(|l| l.split_whitespace().nth(1) == Some(name))
}
fn audio_encode(fmt: &str, quality: &str) -> Vec<String> {
    let t = &tables()["audio"][fmt];
    let mut a = strings(&t["encode"]);
    if !has_encoder(encoder(&a)) && !strings(&t["fallback"]).is_empty() {
        a = strings(&t["fallback"])
    }
    if t["lossless"] == true {
        return a;
    }
    if quality != "best" {
        a.extend(["-b:a".into(), format!("{quality}k")]);
    } else if !strings(&t["best"]).is_empty() {
        a.extend(strings(&t["best"]));
    } else {
        a.extend([
            "-b:a".into(),
            format!("{}k", t["bitrate"].as_str().unwrap_or("192")),
        ]);
    }
    a
}
pub const LOUDNORM: &str = "loudnorm=I=-16:TP=-1.5:LRA=11";
pub fn gif_filter(h: &str) -> String {
    format!(
        "fps=10,scale=-2:{h}:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle"
    )
}
pub fn extension(c: &Choice) -> String {
    tables()[if c.kind == "frames" { "image" } else { &c.kind }][&c.fmt]["ext"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or(&c.fmt)
        .into()
}
pub fn ffmpeg_args(info: &FileInfo, c: &Choice, out: &Path) -> Result<Vec<String>> {
    c.validate()?;
    let mut a = vec!["-hide_banner".into(), "-nostdin".into(), "-y".into()];
    if let Some(s) = c.start {
        a.extend(["-ss".into(), s.to_string()])
    }
    if let Some(e) = c.end {
        a.extend(["-to".into(), e.to_string()])
    }
    a.extend(["-i".into(), info.path.to_string_lossy().into()]);
    let v = info.video.as_ref();
    let ac = info
        .audio
        .as_ref()
        .and_then(|s| s["codec_name"].as_str())
        .unwrap_or("");
    let vc = v.and_then(|s| s["codec_name"].as_str()).unwrap_or("");
    let height = v
        .map(|s| number(&s["width"]).min(number(&s["height"])))
        .unwrap_or(0.);
    let scale = c
        .quality
        .parse::<f64>()
        .ok()
        .filter(|h| *h < height || height == 0.);
    match c.kind.as_str() {
        "video" => {
            let t = &tables()["video"][&c.fmt];
            if t["animated"] == true {
                let h = if c.quality == "best" {
                    "480"
                } else {
                    &c.quality
                };
                if c.fmt == "gif" {
                    a.extend([
                        "-vf".into(),
                        gif_filter(h),
                        "-loop".into(),
                        "0".into(),
                        "-an".into(),
                    ]);
                } else {
                    a.extend([
                        "-vf".into(),
                        format!("fps=15,scale=-2:{h}:flags=lanczos"),
                        "-an".into(),
                    ]);
                    a.extend(strings(&t["encode"]));
                }
            } else {
                a.extend(
                    ["-map", "0:v:0", "-map", "0:a:0?", "-map_metadata", "0"].map(String::from),
                );
                if v.is_some()
                    && scale.is_none()
                    && info.media == "video"
                    && allows(&t["copy_video"], vc)
                {
                    a.extend(["-c:v", "copy"].map(String::from));
                    if vc == "hevc" && ["mp4", "mov", "m4v"].contains(&c.fmt.as_str()) {
                        a.extend(["-tag:v", "hvc1"].map(String::from));
                    }
                } else {
                    if let Some(h) = scale {
                        a.extend([
                            "-vf".into(),
                            format!("scale='if(gt(iw,ih),-2,{h})':'if(gt(iw,ih),{h},-2)'"),
                        ]);
                    }
                    a.extend(strings(&t["encode"]));
                }
                if info.audio.is_some() {
                    if allows(&t["copy_audio"], ac) && !c.loudnorm {
                        a.extend(["-c:a", "copy"].map(String::from));
                    } else if let Some(fmt) = t["audio"].as_str() {
                        a.extend(audio_encode(fmt, "best"));
                        if c.loudnorm {
                            a.extend(["-af".into(), LOUDNORM.into()]);
                        }
                    } else {
                        a.push("-an".into());
                    }
                }
                a.extend(strings(&t["extra"]));
            }
        }
        "audio" => {
            if info.audio.is_none() {
                bail!("File has no audio")
            };
            a.extend(["-vn", "-map", "0:a:0", "-map_metadata", "0"].map(String::from));
            if c.quality == "best" && !c.loudnorm && allows(&tables()["audio"][&c.fmt]["copy"], ac)
            {
                a.extend(["-c:a", "copy"].map(String::from));
            } else {
                a.extend(audio_encode(&c.fmt, &c.quality));
                if c.loudnorm {
                    a.extend(["-af".into(), LOUDNORM.into()]);
                }
            }
        }
        "image" | "frames" => {
            a.extend(["-map", "0:v:0"].map(String::from));
            let mut filters = Vec::new();
            if c.kind == "image" {
                a.extend(["-frames:v", "1", "-update", "1"].map(String::from));
                if c.strip_exif {
                    a.extend(["-map_metadata", "-1"].map(String::from));
                }
                if c.quality == "50" {
                    filters.push("scale=iw/2:-1".into())
                } else if c.quality != "best" {
                    let cap = &c.quality;
                    filters.push(format!(
                        "scale='if(gte(iw,ih),min(iw,{cap}),-1)':'if(gte(iw,ih),-1,min(ih,{cap}))'"
                    ));
                }
                if c.fmt == "gif" {
                    filters.push("split[a][b];[a]palettegen[p];[b][p]paletteuse".into());
                }
            } else {
                a.push("-an".into());
                if c.quality != "best" {
                    filters.push(format!("fps={}", c.quality));
                } else {
                    a.extend(["-fps_mode", "passthrough"].map(String::from));
                }
            }
            if !filters.is_empty() {
                a.extend(["-vf".into(), filters.join(",")]);
            }
            let options = tables()["image"][&c.fmt]["encoders"]
                .as_array()
                .context("Unknown image format")?;
            let chosen = options
                .iter()
                .map(strings)
                .find(|a| has_encoder(encoder(a)));
            a.extend(chosen.context(format!(
                "No FFmpeg encoder for {}. HEIC/JXL require native image tools.",
                c.fmt
            ))?);
        }
        _ => bail!("Unknown media kind"),
    }
    a.push(out.to_string_lossy().into());
    Ok(a)
}
pub fn convert(info: &FileInfo, c: &Choice, cancel: &Cancel, report: &Reporter) -> Result<PathBuf> {
    c.validate()?;
    let parent = info.path.parent().unwrap_or(Path::new("."));
    let work = tempfile::Builder::new()
        .prefix(".gingertea-")
        .tempdir_in(parent)?;
    let stem = info.path.file_stem().unwrap_or_default().to_string_lossy();
    if c.chapters {
        let chapters = info.raw["chapters"]
            .as_array()
            .filter(|a| !a.is_empty())
            .context("This file has no chapters")?;
        let folder = work.path().join("chapters");
        std::fs::create_dir(&folder)?;
        for (i, ch) in chapters.iter().enumerate() {
            let mut choice = c.clone();
            choice.chapters = false;
            choice.start = Some(number(&ch["start_time"]));
            choice.end = Some(number(&ch["end_time"]));
            let name = format!(
                "{:02} {}.{}",
                i + 1,
                state::sanitize(ch["tags"]["title"].as_str().unwrap_or("Chapter")),
                extension(c)
            );
            let out = folder.join(name);
            run_ffmpeg(info, &choice, &out, cancel, report)?;
        }
        return state::publish(&folder, &parent.join(format!("{stem} chapters")));
    }
    let name = if c.kind == "frames" {
        format!("{stem} frames")
    } else {
        format!("{stem}.{}", extension(c))
    };
    let out = work.path().join(&name);
    if c.kind == "frames" {
        std::fs::create_dir(&out)?;
    }
    let actual = if c.kind == "frames" {
        out.join(format!("{stem} %04d.{}", extension(c)))
    } else {
        out.clone()
    };
    run_ffmpeg(info, c, &actual, cancel, report)?;
    if !out.exists() {
        bail!("Media tool produced no output")
    }
    if let Some(kb) = c.max_kb.filter(|n| *n > 0)
        && c.kind == "image"
        && out.metadata()?.len() > kb * 1024
    {
        fit_image(&out, kb, cancel, report)?;
    }
    state::publish(&out, &parent.join(name))
}
fn fit_image(path: &Path, kb: u64, cancel: &Cancel, report: &Reporter) -> Result<()> {
    let tmp = tempfile::Builder::new()
        .prefix(".fit-")
        .tempdir_in(path.parent().unwrap())?;
    let candidate = tmp.path().join(path.file_name().unwrap());
    for q in [4, 8, 12, 18, 25, 31] {
        let mut cmd = command("ffmpeg");
        cmd.args(["-v", "error", "-y", "-i"])
            .arg(path)
            .args(["-frames:v", "1", "-q:v", &q.to_string()])
            .arg(&candidate);
        run(cmd, cancel, report)?;
        if candidate.metadata()?.len() < path.metadata()?.len() {
            std::fs::copy(&candidate, path)?;
        }
        if path.metadata()?.len() <= kb * 1024 {
            return Ok(());
        }
    }
    bail!("Image could not fit the requested {kb} KB limit")
}
fn decode_image(path: &Path, out: &Path) -> Result<()> {
    let ext = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let mut cmd = if ext == "jxl" {
        let mut c = command("djxl");
        c.arg(path).arg(out);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = Command::new("sips");
        c.args(["-s", "format", "png"])
            .arg(path)
            .arg("--out")
            .arg(out);
        c
    } else {
        let mut c = command("heif-convert");
        c.arg(path).arg(out);
        c
    };
    let result = cmd
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .context("Install jpeg-xl (cjxl/djxl) or libheif tools for this image format")?;
    if !result.status.success() {
        bail!("{}", String::from_utf8_lossy(&result.stderr))
    }
    Ok(())
}
fn run_ffmpeg(
    info: &FileInfo,
    c: &Choice,
    out: &Path,
    cancel: &Cancel,
    report: &Reporter,
) -> Result<()> {
    let ext = info
        .path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if ["heic", "heif", "hif", "jxl"].contains(&ext.as_str()) {
        let work = tempfile::tempdir()?;
        let decoded = work.path().join("decoded.png");
        decode_image(&info.path, &decoded)?;
        let mut source = info.clone();
        source.path = decoded;
        return run_ffmpeg(&source, c, out, cancel, report);
    }
    if c.kind == "image" && ["heic", "jxl"].contains(&c.fmt.as_str()) {
        let work = tempfile::tempdir()?;
        let png = work.path().join("prepared.png");
        let mut intermediate = c.clone();
        intermediate.fmt = "png".into();
        run_ffmpeg(info, &intermediate, &png, cancel, report)?;
        let cmd = if c.fmt == "jxl" {
            let mut cmd = command("cjxl");
            cmd.arg(&png).arg(out).args(["-q", "90"]);
            cmd
        } else if cfg!(target_os = "macos") {
            let mut cmd = Command::new("sips");
            cmd.args(["-s", "format", "heic", "-s", "formatOptions", "85"])
                .arg(&png)
                .arg("--out")
                .arg(out);
            cmd
        } else {
            let mut cmd = command("heif-enc");
            cmd.arg(&png).arg("-o").arg(out).args(["-q", "85"]);
            cmd
        };
        return run(cmd, cancel, report);
    }
    let mut cmd = command("ffmpeg");
    cmd.args(["-progress", "pipe:1", "-nostats", "-loglevel", "error"]);
    cmd.args(ffmpeg_args(info, c, out)?);
    run(cmd, cancel, report)
}
pub fn thumbnail(url: &str) -> Result<Vec<Vec<(u8, u8, u8)>>> {
    let mut cmd = command("ffmpeg");
    if url.starts_with("http://") || url.starts_with("https://") {
        cmd.args([
            "-rw_timeout",
            "8000000",
            "-user_agent",
            "Mozilla/5.0 gingertea",
        ]);
    }
    cmd.args([
        "-v",
        "error",
        "-i",
        url,
        "-vf",
        "scale=36:18:force_original_aspect_ratio=increase,crop=36:18",
        "-frames:v",
        "1",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgb24",
        "pipe:1",
    ]);
    let data = capture_bytes(cmd)?;
    if data.len() != 36 * 18 * 3 {
        bail!("No thumbnail")
    }
    Ok(data
        .chunks(36 * 3)
        .map(|row| row.chunks(3).map(|p| (p[0], p[1], p[2])).collect())
        .collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn copy_and_scale() {
        let info=parse_probe(Path::new("x.mp4"),json!({"format":{"format_name":"mov,mp4","duration":"10"},"streams":[{"codec_type":"video","codec_name":"h264","width":1920,"height":1080},{"codec_type":"audio","codec_name":"aac"}]})).unwrap();
        let mut c = Choice::default();
        let a = ffmpeg_args(&info, &c, Path::new("out.mp4")).unwrap();
        assert!(a.windows(2).any(|a| a == ["-c:v", "copy"]));
        c.quality = "360".into();
        let a = ffmpeg_args(&info, &c, Path::new("out.mp4")).unwrap();
        assert!(!a.windows(2).any(|a| a == ["-c:v", "copy"]));
        assert!(a.iter().any(|s| s.contains("360")));
    }
    #[test]
    fn clip_validation() {
        let mut c = Choice {
            start: Some(5.),
            end: Some(2.),
            ..Choice::default()
        };
        assert!(c.validate().is_err());
        c.end = Some(10.);
        assert!(c.validate().is_ok());
    }
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    #[test]
    fn python_planner_parity() {
        let cases: Value = serde_json::from_str(include_str!("planner_cases.json")).unwrap();
        for case in cases.as_array().unwrap() {
            let c: Choice = serde_json::from_value(case["choice"].clone()).unwrap();
            // Codec availability differs across hosts. Skip image cases requiring unavailable encoders.
            let info = parse_probe(Path::new("input.mp4"), case["raw"].clone()).unwrap();
            let result = ffmpeg_args(&info, &c, Path::new("out"));
            if c.kind == "image" && result.is_err() {
                continue;
            }
            let args = result.unwrap();
            let mut expected: Vec<String> =
                serde_json::from_value(case["expected"].clone()).unwrap();
            // The oracle assumes every encoder. Audio fallbacks also apply to
            // audio streams inside videos, not only standalone audio output.
            for target in tables()["audio"].as_object().unwrap().values() {
                let primary = strings(&target["encode"]);
                let fallback = strings(&target["fallback"]);
                if !primary.is_empty()
                    && !fallback.is_empty()
                    && !has_encoder(encoder(&primary))
                    && let Some(i) = expected.windows(primary.len()).position(|v| v == primary)
                {
                    expected.splice(i..i + primary.len(), fallback);
                }
            }
            if c.kind == "image" {
                let candidates = tables()["image"][&c.fmt]["encoders"].as_array().unwrap();
                let primary = strings(&candidates[0]);
                let chosen = candidates
                    .iter()
                    .map(strings)
                    .find(|a| has_encoder(encoder(a)))
                    .unwrap();
                if chosen != primary
                    && let Some(i) = expected.windows(primary.len()).position(|v| v == primary)
                {
                    expected.splice(i..i + primary.len(), chosen);
                }
            }
            assert_eq!(args, expected, "{} {} {}", c.kind, c.fmt, c.quality);
        }
    }
}

#[cfg(all(test, unix))]
mod cancellation_tests {
    use super::*;
    #[test]
    fn cancels_worker_process_group() {
        let cancel = Arc::new(AtomicBool::new(false));
        let c = cancel.clone();
        let report: Reporter = Arc::new(|_| {});
        let thread = std::thread::spawn(move || {
            let mut cmd = Command::new("sh");
            cmd.args(["-c", "sleep 30 & wait"]);
            run(cmd, &c, &report)
        });
        std::thread::sleep(Duration::from_millis(100));
        let start = std::time::Instant::now();
        cancel.store(true, Ordering::Relaxed);
        assert!(thread.join().unwrap().is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}
