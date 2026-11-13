use anyhow::{Context, Result, bail};
use directories::{ProjectDirs, UserDirs};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
static HISTORY_LOCK: Mutex<()> = Mutex::new(());
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct Settings {
    pub download_dir: String,
    pub last_kind: String,
    pub last_video_format: String,
    pub last_video_quality: String,
    pub last_audio_format: String,
    pub last_audio_quality: String,
    pub last_image_format: String,
    pub last_image_quality: String,
    pub last_frames_format: String,
    pub last_frames_quality: String,
    pub cookies_browser: String,
    pub theme: String,
    pub animations: bool,
    pub parallel: usize,
    pub filename_template: String,
    pub subtitle_mode: String,
    pub subtitle_langs: String,
    pub sponsorblock: String,
    pub split_chapters: bool,
    pub loudnorm: bool,
    pub image_max_kb: String,
    pub strip_exif: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            download_dir: UserDirs::new()
                .and_then(|u| u.download_dir().map(|p| p.to_string_lossy().into_owned()))
                .unwrap_or_else(|| ".".into()),
            last_kind: "video".into(),
            last_video_format: "mp4".into(),
            last_video_quality: "best".into(),
            last_audio_format: "mp3".into(),
            last_audio_quality: "best".into(),
            last_image_format: "jpg".into(),
            last_image_quality: "best".into(),
            last_frames_format: "png".into(),
            last_frames_quality: "1".into(),
            cookies_browser: String::new(),
            theme: "masala-chai".into(),
            animations: true,
            parallel: 1,
            filename_template: "{title} [{id}]".into(),
            subtitle_mode: "off".into(),
            subtitle_langs: String::new(),
            sponsorblock: "off".into(),
            split_chapters: false,
            loudnorm: false,
            image_max_kb: String::new(),
            strip_exif: false,
        }
    }
}
pub fn config_dir() -> PathBuf {
    if let Ok(p) = std::env::var("GINGERTEA_CONFIG_DIR") {
        return p.into();
    }
    platform_config().unwrap_or_else(|| PathBuf::from("."))
}
pub fn data_dir() -> PathBuf {
    if let Ok(p) = std::env::var("GINGERTEA_DATA_DIR") {
        return p.into();
    }
    platform_data().unwrap_or_else(|| PathBuf::from("."))
}
fn platform_config() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("gingertea"))
    } else {
        ProjectDirs::from("", "", "gingertea").map(|d| d.config_dir().to_owned())
    }
}
fn platform_data() -> Option<PathBuf> {
    if cfg!(windows) {
        platform_config()
    } else {
        ProjectDirs::from("", "", "gingertea").map(|d| d.data_dir().to_owned())
    }
}
impl Settings {
    pub fn load() -> Self {
        let mut s: Self = fs::read_to_string(config_dir().join("config.toml"))
            .ok()
            .and_then(|v| toml::from_str(&v).ok())
            .unwrap_or_default();
        s.parallel = s.parallel.clamp(1, 4);
        s
    }
    pub fn save(&self) -> Result<()> {
        let dir = config_dir();
        fs::create_dir_all(&dir)?;
        let mut tmp = tempfile::NamedTempFile::new_in(&dir)?;
        std::io::Write::write_all(&mut tmp, toml::to_string_pretty(self)?.as_bytes())?;
        tmp.persist(dir.join("config.toml"))?;
        Ok(())
    }
    pub fn download_path(&self) -> PathBuf {
        expand(&self.download_dir)
    }
}
pub fn expand(p: &str) -> PathBuf {
    if (p == "~" || p.starts_with("~/"))
        && let Some(d) = UserDirs::new()
    {
        return d.home_dir().join(p.strip_prefix("~/").unwrap_or(""));
    }
    PathBuf::from(p)
}
pub fn sanitize(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() && !"<>:\"\\|?*".contains(*c))
        .take(180)
        .collect::<String>()
        .trim_matches([' ', '.'])
        .to_string()
}
pub fn render_name(template: &str, fields: &Value) -> Result<PathBuf> {
    let mut out = String::new();
    let mut chars = template.chars();
    while let Some(c) = chars.next() {
        if c == '{' {
            let key: String = chars.by_ref().take_while(|c| *c != '}').collect();
            if !["title", "id", "channel", "date", "index", "ext"].contains(&key.as_str()) {
                bail!("Unknown filename field: {key}")
            }
            out.push_str(fields[&key].as_str().unwrap_or(""))
        } else {
            out.push(c)
        }
    }
    let parts: Vec<String> = out
        .split('/')
        .map(sanitize)
        .filter(|s| !s.is_empty() && s != "..")
        .collect();
    Ok(if parts.is_empty() {
        PathBuf::from("download")
    } else {
        parts.iter().collect()
    })
}
pub fn unique_path(p: &Path) -> PathBuf {
    if !p.exists() {
        return p.to_owned();
    }
    for n in 2.. {
        let stem = p.file_stem().unwrap_or_default().to_string_lossy();
        let ext = p
            .extension()
            .map(|s| format!(".{}", s.to_string_lossy()))
            .unwrap_or_default();
        let q = p.with_file_name(format!("{stem} ({n}){ext}"));
        if !q.exists() {
            return q;
        }
    }
    unreachable!()
}
pub fn publish(src: &Path, dest: &Path) -> Result<PathBuf> {
    fs::create_dir_all(dest.parent().context("No output directory")?)?;
    let dest = unique_path(dest);
    if src.is_dir() {
        fs::rename(src, &dest)?;
    } else {
        // hard link refuses to overwrite even if another job publishes simultaneously
        let mut target = dest;
        loop {
            match fs::hard_link(src, &target) {
                Ok(()) => {
                    fs::remove_file(src)?;
                    return Ok(target);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    target = unique_path(&target)
                }
                Err(e) => return Err(e.into()),
            }
        }
    }
    Ok(dest)
}
pub fn history() -> Vec<Value> {
    let mut v: Vec<_> = fs::read_to_string(data_dir().join("history.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|s| serde_json::from_str::<Value>(s).ok())
        .filter(|v| v["id"].is_string())
        .collect();
    v.reverse();
    v
}
pub fn remember(url: &str, title: &str, path: &Path) -> Result<()> {
    use std::io::Write;
    let _lock = HISTORY_LOCK.lock().unwrap();
    fs::create_dir_all(data_dir())?;
    let now = chrono::Utc::now();
    let item = json!({"id":format!("{}",now.timestamp_nanos_opt().unwrap_or_default()),"url":url,"title":title,"path":path,"saved":now.to_rfc3339()});
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(data_dir().join("history.jsonl"))?;
    writeln!(f, "{item}")?;
    Ok(())
}
pub fn forget(id: &str) -> Result<()> {
    let _lock = HISTORY_LOCK.lock().unwrap();
    let mut rows = history();
    rows.reverse();
    let text = rows
        .iter()
        .filter(|v| v["id"] != id)
        .map(|v| format!("{v}\n"))
        .collect::<String>();
    fs::write(data_dir().join("history.jsonl"), text)?;
    Ok(())
}
pub fn parse_time(text: &str) -> Result<Option<f64>> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let n = if text.contains(':') {
        let mut n = 0.;
        for part in text.split(':') {
            n = n * 60. + part.parse::<f64>()?;
        }
        n
    } else if text.contains(['h', 'm', 's']) {
        let mut n = 0.;
        let mut part = String::new();
        for c in text.chars() {
            if c.is_ascii_digit() || c == '.' {
                part.push(c)
            } else {
                let v = part.parse::<f64>()?;
                n += v * match c {
                    'h' => 3600.,
                    'm' => 60.,
                    's' => 1.,
                    _ => bail!("Invalid time"),
                };
                part.clear()
            }
        }
        if !part.is_empty() {
            bail!("Invalid time")
        }
        n
    } else {
        text.parse()?
    };
    if !n.is_finite() || n < 0. {
        bail!("Time must be finite and nonnegative")
    }
    Ok(Some(n))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn time() {
        assert_eq!(parse_time("1h2m3s").unwrap(), Some(3723.));
        assert_eq!(parse_time("1:30").unwrap(), Some(90.));
        assert!(parse_time("NaN").is_err());
    }
    #[test]
    fn filenames() {
        assert!(render_name("{oops}", &json!({})).is_err());
        assert_eq!(
            render_name("../../{title}", &json!({"title":"hello"})).unwrap(),
            PathBuf::from("hello")
        );
    }
}
