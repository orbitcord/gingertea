use crate::{media, state};
use anyhow::{Context, Result, bail};
use std::{fs, path::Path, process::Command};
pub fn setup() -> Result<()> {
    let tools = state::data_dir().join("tools");
    fs::create_dir_all(&tools)?;
    for (name, flag) in [
        ("yt-dlp", "--version"),
        ("ffmpeg", "-version"),
        ("ffprobe", "-version"),
        ("deno", "--version"),
    ] {
        let mut c = media::command(name);
        c.arg(flag);
        if let Ok(v) = media::capture(c) {
            println!("{name}: {}", v.lines().next().unwrap_or("ready"));
            continue;
        }
        let work = tempfile::tempdir_in(&tools)?;
        if name == "yt-dlp" {
            let asset = if cfg!(windows) {
                "yt-dlp.exe"
            } else if cfg!(target_os = "macos") {
                "yt-dlp_macos"
            } else if cfg!(target_arch = "aarch64") {
                "yt-dlp_linux_aarch64"
            } else {
                "yt-dlp_linux"
            };
            let dest = work.path().join(if cfg!(windows) {
                "yt-dlp.exe"
            } else {
                "yt-dlp"
            });
            fetch(
                &format!("https://github.com/yt-dlp/yt-dlp/releases/latest/download/{asset}"),
                &dest,
            )?;
            install(&dest, &tools)?;
        } else if name == "ffmpeg" || name == "ffprobe" {
            let key = media::platform_key();
            let zip = work.path().join("ffmpeg.zip");
            fetch(
                &format!("https://github.com/zackees/ffmpeg_bins/raw/main/v8.0/{key}.zip"),
                &zip,
            )?;
            unzip(&zip, work.path())?;
            for tool in ["ffmpeg", "ffprobe"] {
                let file = if cfg!(windows) {
                    format!("{tool}.exe")
                } else {
                    tool.into()
                };
                install(&work.path().join(&key).join(file), &tools)?;
            }
        } else {
            let target = match (std::env::consts::OS, std::env::consts::ARCH) {
                ("macos", "aarch64") => "aarch64-apple-darwin",
                ("macos", _) => "x86_64-apple-darwin",
                ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
                ("linux", _) => "x86_64-unknown-linux-gnu",
                ("windows", _) => "x86_64-pc-windows-msvc",
                _ => bail!("Unsupported platform"),
            };
            let zip = work.path().join("deno.zip");
            fetch(
                &format!(
                    "https://github.com/denoland/deno/releases/latest/download/deno-{target}.zip"
                ),
                &zip,
            )?;
            unzip(&zip, work.path())?;
            install(
                &work
                    .path()
                    .join(if cfg!(windows) { "deno.exe" } else { "deno" }),
                &tools,
            )?;
        }
        println!("Installed {name}");
    }
    Ok(())
}
fn fetch(url: &str, dest: &Path) -> Result<()> {
    let status = Command::new("curl")
        .args([
            "--fail",
            "--location",
            "--retry",
            "2",
            "--connect-timeout",
            "20",
            "--max-time",
            "300",
            "--output",
        ])
        .arg(dest)
        .arg(url)
        .status()
        .context("curl is required for dependency setup")?;
    if !status.success() {
        bail!("Download failed: {url}")
    }
    Ok(())
}
fn unzip(zip: &Path, dir: &Path) -> Result<()> {
    let status = if cfg!(windows) {
        Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "Expand-Archive -LiteralPath $env:GINGERTEA_ZIP -DestinationPath $env:GINGERTEA_UNPACK",
            ])
            .env("GINGERTEA_ZIP",zip)
            .env("GINGERTEA_UNPACK",dir)
            .status()?
    } else {
        Command::new("unzip")
            .arg("-q")
            .arg(zip)
            .arg("-d")
            .arg(dir)
            .status()?
    };
    if !status.success() {
        bail!("Could not unpack {}", zip.display())
    }
    Ok(())
}
fn install(src: &Path, dir: &Path) -> Result<()> {
    let dest = dir.join(src.file_name().context("Missing tool name")?);
    fs::copy(src, &dest)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dest, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}
