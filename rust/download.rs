use crate::{
    media::{self, Cancel, Choice, Reporter},
    state::{self, Settings},
};
use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
pub fn probe(input: &str, s: &Settings) -> Result<Value> {
    let url = normalize(input);
    let mut c = media::command("yt-dlp");
    c.args([
        "--ignore-config",
        "--dump-single-json",
        "--flat-playlist",
        "--no-warnings",
        "--skip-download",
        "--socket-timeout",
        "20",
    ]);
    if !s.cookies_browser.is_empty() {
        c.args(["--cookies-from-browser", &s.cookies_browser]);
    }
    c.arg("--").arg(url);
    Ok(serde_json::from_str(&media::capture(c)?)?)
}
/// Prefer a small widescreen image, then keep alternatives for failed URLs.
pub fn thumbnail_urls(info: &Value) -> Vec<String> {
    fn append(info: &Value, urls: &mut Vec<String>) {
        let mut candidates = info["thumbnails"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|thumb| {
                let url = thumb["url"].as_str()?.trim();
                if url.is_empty() {
                    return None;
                }
                let width = media::number(&thumb["width"]);
                let height = media::number(&thumb["height"]);
                let wide = width >= 300. && height > 0. && (width / height - 16. / 9.).abs() < 0.1;
                Some((!wide, url.contains(".webp"), width as u64, url.to_owned()))
            })
            .collect::<Vec<_>>();
        candidates.sort();
        let alternatives = candidates.into_iter().map(|(_, _, _, url)| url).chain(
            info["thumbnail"]
                .as_str()
                .filter(|url| !url.trim().is_empty())
                .map(str::to_owned),
        );
        for url in alternatives {
            let url = if url.starts_with("//") {
                format!("https:{url}")
            } else {
                url
            };
            if !urls.contains(&url) {
                urls.push(url);
            }
        }
    }
    let mut urls = Vec::new();
    append(info, &mut urls);
    if let Some(entries) = info["entries"].as_array() {
        for entry in entries.iter().take(3) {
            append(entry, &mut urls);
        }
    }
    urls
}
pub fn normalize(input: &str) -> String {
    let text = input.trim();
    if !text.contains("://") && !text.starts_with("ytsearch") {
        format!("ytsearch10:{text}")
    } else if text.contains("youtube.com/@")
        && !text
            .split("youtube.com/")
            .nth(1)
            .unwrap_or("")
            .contains('/')
    {
        format!("{}/videos", text.trim_end_matches('/'))
    } else {
        text.into()
    }
}
pub fn template(s: &str) -> Result<String> {
    state::render_name(s, &json!({}))?;
    let safe = s
        .split('/')
        .map(str::trim)
        .filter(|p| !p.is_empty() && *p != ".." && *p != ".")
        .collect::<Vec<_>>()
        .join("/");
    let mut out = safe.replace('%', "%%").replace('\\', "");
    for (a, b) in [
        ("title", "title"),
        ("id", "id"),
        ("channel", "channel"),
        ("date", "upload_date"),
        ("index", "playlist_index"),
        ("ext", "ext"),
    ] {
        out = out.replace(&format!("{{{a}}}"), &format!("%({b})s"));
    }
    if !s.contains("{ext}") {
        out.push_str(".%(ext)s")
    }
    Ok(out)
}
pub fn args(c: &Choice, s: &Settings, work: &Path) -> Result<Vec<String>> {
    c.validate()?;
    if !["video", "audio"].contains(&c.kind.as_str()) {
        bail!("Download kind must be video or audio")
    }
    let mut a = vec![
        "--ignore-config",
        "--no-warnings",
        "--no-overwrites",
        "--windows-filenames",
        "--newline",
        "--progress",
        "--progress-template",
        "download:%(progress._percent_str)s %(progress._speed_str)s ETA %(progress._eta_str)s",
        "--print",
        "after_move:GINGERTEA_PATH:%(filepath)s",
        "--paths",
    ]
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();
    a.push(work.to_string_lossy().into());
    let ffmpeg = media::executable("ffmpeg");
    if ffmpeg.is_absolute() {
        a.extend(["--ffmpeg-location".into(), ffmpeg.to_string_lossy().into()]);
    }
    let deno = media::executable("deno");
    a.extend([
        "--js-runtimes".into(),
        if deno.is_absolute() {
            format!("deno:{}", deno.display())
        } else {
            "deno".into()
        },
    ]);
    a.extend(["-o".into(), template(&s.filename_template)?]);
    a.extend([
        "--download-archive".into(),
        state::data_dir()
            .join("archive.txt")
            .to_string_lossy()
            .into(),
    ]);
    if !s.cookies_browser.is_empty() {
        a.extend(["--cookies-from-browser".into(), s.cookies_browser.clone()]);
    }
    if c.kind == "audio" {
        a.extend(["-f", "ba/b", "-x", "--audio-format"].map(String::from));
        a.push(if c.fmt == "ogg" {
            "vorbis".into()
        } else {
            c.fmt.clone()
        });
        a.extend([
            "--audio-quality".into(),
            if c.quality == "best" {
                "0".into()
            } else {
                format!("{}K", c.quality)
            },
        ]);
    } else if c.fmt == "gif" {
        a.extend(
            [
                "-f",
                "bv*/b",
                "--recode-video",
                "gif",
                "--postprocessor-args",
            ]
            .map(String::from),
        );
        a.push(format!(
            "VideoConvertor:-vf {} -loop 0 -an",
            shell_words::quote(&media::gif_filter(if c.quality == "best" {
                "480"
            } else {
                &c.quality
            }))
        ));
    } else {
        a.extend(["-f", "bv*+ba/b", "-S"].map(String::from));
        let res = if c.quality == "best" {
            "res".into()
        } else {
            format!("res:{}", c.quality)
        };
        a.push(match c.fmt.as_str() {
            "mp4" | "mov" if c.quality == "best" => format!("vcodec:h264,acodec:aac,{res}"),
            "mp4" | "mov" => format!("{res},vcodec:h264,acodec:aac"),
            "webm" => format!("{res},vcodec:vp9,acodec:opus"),
            _ => res,
        });
        a.extend([
            "--merge-output-format".into(),
            format!("{}/mkv", c.fmt),
            if c.fmt == "mp4" || c.fmt == "mov" {
                "--remux-video".into()
            } else {
                "--recode-video".into()
            },
            c.fmt.clone(),
        ]);
    }
    if c.fmt != "gif" {
        a.extend(["--embed-metadata", "--embed-chapters"].map(String::from));
    }
    if ["mp4", "mov", "mkv", "m4a", "mp3", "flac", "opus", "ogg"].contains(&c.fmt.as_str()) {
        a.extend(["--embed-thumbnail", "--convert-thumbnails", "jpg"].map(String::from));
    }
    if c.start.is_some() || c.end.is_some() {
        a.extend([
            "--download-sections".into(),
            format!(
                "*{}-{}",
                c.start.unwrap_or(0.),
                c.end.map(|e| e.to_string()).unwrap_or_else(|| "inf".into())
            ),
            "--force-keyframes-at-cuts".into(),
        ]);
    }
    if c.subtitles != "off" && !c.languages.is_empty() {
        a.extend(["--write-subs", "--write-auto-subs", "--sub-langs"].map(String::from));
        a.push(c.languages.clone());
        a.extend(["--convert-subs", "srt"].map(String::from));
        if c.subtitles == "embed" {
            a.push("--embed-subs".into());
        }
    }
    let categories = "sponsor,intro,outro,selfpromo,preview,filler,interaction";
    match c.sponsorblock.as_str() {
        "cut" => a.extend(["--sponsorblock-remove".into(), categories.into()]),
        "chapters" => a.extend(["--sponsorblock-mark".into(), categories.into()]),
        _ => (),
    };
    if c.chapters {
        a.push("--split-chapters".into());
    }
    if c.loudnorm {
        a.extend([
            "--postprocessor-args".into(),
            format!(
                "{}:-af {}",
                if c.kind == "audio" {
                    "ExtractAudio"
                } else {
                    "VideoConvertor"
                },
                media::LOUDNORM
            ),
        ]);
        if c.kind == "video" {
            a.extend(["--recode-video".into(), c.fmt.clone()]);
        }
    }
    Ok(a)
}
pub fn download(
    url: &str,
    c: &Choice,
    s: &Settings,
    cancel: &Cancel,
    report: &Reporter,
) -> Result<Vec<PathBuf>> {
    let info = probe(url, s)?;
    let playlist = info["entries"].is_array();
    let mut settings = s.clone();
    if playlist {
        let title = info["title"].as_str().unwrap_or("playlist");
        let channel = info["channel"].as_str().unwrap_or(title);
        let title = if [
            "videos", "shorts", "streams", "uploads", "playlist", "featured",
        ]
        .contains(&title.to_lowercase().as_str())
        {
            channel
        } else {
            title
        };
        settings.download_dir = s
            .download_path()
            .join(state::sanitize(title))
            .to_string_lossy()
            .into();
    }
    download_item(url, c, &settings, cancel, report, playlist, playlist)
}
pub fn download_item(
    url: &str,
    c: &Choice,
    s: &Settings,
    cancel: &Cancel,
    report: &Reporter,
    playlist: bool,
    use_archive: bool,
) -> Result<Vec<PathBuf>> {
    let dir = s.download_path();
    fs::create_dir_all(&dir)?;
    fs::create_dir_all(state::data_dir())?;
    let work = tempfile::Builder::new()
        .prefix(".gingertea-")
        .tempdir_in(&dir)?;
    // Stage the archive with outputs: failures/cancellation must not mark unpublished files downloaded.
    let archive = state::data_dir().join("archive.txt");
    let staged = work.path().join("archive.txt");
    if use_archive && archive.exists() {
        fs::copy(&archive, &staged)?;
    }
    let mut a = args(c, s, work.path())?;
    if let Some(i) = a.iter().position(|v| v == "--download-archive") {
        if use_archive {
            a[i + 1] = staged.to_string_lossy().into();
        } else {
            a.drain(i..i + 2);
        }
    }
    a.push(
        if playlist {
            "--yes-playlist"
        } else {
            "--no-playlist"
        }
        .into(),
    );
    let paths = Arc::new(Mutex::new(Vec::new()));
    let p = paths.clone();
    let r = report.clone();
    let hook: Reporter = Arc::new(move |line| {
        if let Some(path) = line.strip_prefix("GINGERTEA_PATH:") {
            p.lock().unwrap().push(PathBuf::from(path));
        } else {
            r(line)
        }
    });
    let mut cmd = media::command("yt-dlp");
    cmd.args(a).arg("--").arg(normalize(url));
    media::run(cmd, cancel, &hook)?;
    media::cancelled(cancel)?;
    let mut saved = Vec::new();
    fn collect(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        for e in fs::read_dir(dir)? {
            let p = e?.path();
            if p.is_dir() {
                collect(&p, files)?
            } else {
                files.push(p)
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    collect(work.path(), &mut files)?;
    let sidecar = |p: &Path| {
        ["jpg", "png", "webp", "srt", "vtt"]
            .contains(&p.extension().unwrap_or_default().to_string_lossy().as_ref())
    };
    files.sort_by_key(|p| sidecar(p));
    let mut renamed: Vec<(PathBuf, PathBuf)> = Vec::new();
    for src in files {
        if src == staged || src.extension().is_some_and(|s| s == "part" || s == "ytdl") {
            continue;
        }
        let rel = src.strip_prefix(work.path())?;
        let mut target = dir.join(rel);
        if sidecar(&src) {
            for (old, new) in &renamed {
                if old.parent() == src.parent() {
                    let stem = old.file_stem().unwrap_or_default().to_string_lossy();
                    let name = src.file_name().unwrap_or_default().to_string_lossy();
                    if let Some(suffix) = name.strip_prefix(&format!("{stem}.")) {
                        target = new.with_file_name(format!(
                            "{}.{}",
                            new.file_stem().unwrap_or_default().to_string_lossy(),
                            suffix
                        ));
                        break;
                    }
                }
            }
        }
        let published = state::publish(&src, &target)?;
        if !["jpg", "png", "webp", "srt", "vtt"].contains(
            &published
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .as_ref(),
        ) {
            state::remember(
                url,
                published
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .as_ref(),
                &published,
            )?;
            renamed.push((src.clone(), published.clone()));
            saved.push(published);
        }
    }
    if saved.is_empty() {
        if staged.exists() {
            report("Already downloaded; archive skipped this item".into());
            return Ok(saved);
        }
        bail!("No output files produced")
    }
    if staged.exists() {
        use std::io::Write;
        static LOCK: Mutex<()> = Mutex::new(());
        let _lock = LOCK.lock().unwrap();
        let existing = fs::read_to_string(&archive).unwrap_or_default();
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&archive)?;
        for line in fs::read_to_string(&staged)?.lines() {
            if !existing.lines().any(|l| l == line) {
                writeln!(f, "{line}")?;
            }
        }
    }
    Ok(saved)
}
pub fn update(cancel: &Cancel, report: &Reporter) -> Result<()> {
    let mut cmd = media::command("yt-dlp");
    cmd.arg("-U");
    media::run(cmd, cancel, report)
}
pub fn sites() -> Result<Vec<String>> {
    let mut cmd = media::command("yt-dlp");
    cmd.arg("--list-extractors");
    Ok(media::capture(cmd)?.lines().map(String::from).collect())
}
#[cfg(test)]
mod tests {
    #[test]
    fn playlist_thumbnail_candidates_and_item_fallback() {
        let info = serde_json::json!({"thumbnails":[
            {"url":"https://image.test/large.jpg","width":1280,"height":720},
            {"url":"https://image.test/small.webp","width":320,"height":180},
            {"url":"https://image.test/small.jpg","width":320,"height":180}
        ],"entries":[{"thumbnails":[{"url":"https://image.test/item.jpg"}]}]});
        let urls = super::thumbnail_urls(&info);
        assert_eq!(urls[0], "https://image.test/small.jpg");
        assert!(urls.contains(&"https://image.test/item.jpg".to_owned()));
        assert_eq!(
            super::thumbnail_urls(
                &serde_json::json!({"entries":[{"thumbnail":"//image.test/fallback.jpg"}]})
            ),
            vec!["https://image.test/fallback.jpg"]
        );
        assert!(super::thumbnail_urls(&serde_json::json!({"thumbnail":""})).is_empty());
    }

    use super::*;
    #[test]
    fn mapping() {
        assert_eq!(
            template("{channel}/{title} [{id}]").unwrap(),
            "%(channel)s/%(title)s [%(id)s].%(ext)s"
        );
        assert!(template("{unknown}").is_err());
    }
    #[test]
    fn download_choices() {
        let c = Choice {
            kind: "audio".into(),
            fmt: "mp3".into(),
            quality: "192".into(),
            start: Some(1.),
            end: Some(3.),
            ..Choice::default()
        };
        let a = args(&c, &Settings::default(), Path::new("/tmp")).unwrap();
        assert!(a.contains(&"192K".into()));
        assert!(a.contains(&"*1-3".into()));
    }
    #[test]
    fn query() {
        assert_eq!(normalize("tea music"), "ytsearch10:tea music");
    }
}
