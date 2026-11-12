//! Real media and CLI acceptance checks. Requires FFmpeg, as does the app.
use std::{
    path::Path,
    process::{Command, Output},
};
fn app(args: &[&str], dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gingertea"))
        .args(args)
        .env("GINGERTEA_CONFIG_DIR", dir.join("config"))
        .env("GINGERTEA_DATA_DIR", dir.join("data"))
        .output()
        .unwrap()
}
fn fixture(dir: &Path) -> std::path::PathBuf {
    let p = dir.join("source.mp4");
    let o = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=320x240:rate=10",
            "-f",
            "lavfi",
            "-i",
            "sine=duration=2",
            "-t",
            "2",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-shortest",
        ])
        .arg(&p)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    p
}
fn path(out: Output) -> std::path::PathBuf {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}
#[test]
fn cli_errors() {
    let t = tempfile::tempdir().unwrap();
    assert!(
        !app(&["convert", "missing.mp4", "-f", "nonsense"], t.path())
            .status
            .success()
    );
    assert!(
        !app(
            &[
                "convert",
                "missing.mp4",
                "-f",
                "mp4",
                "--start",
                "10",
                "--end",
                "2"
            ],
            t.path()
        )
        .status
        .success()
    );
    assert!(app(&["--version"], t.path()).status.success());
}
#[test]
fn convert_preserves_original_and_never_overwrites() {
    let t = tempfile::tempdir().unwrap();
    let src = fixture(t.path());
    let before = std::fs::read(&src).unwrap();
    let a = path(app(
        &["convert", src.to_str().unwrap(), "-f", "mp4"],
        t.path(),
    ));
    assert_ne!(src, a);
    assert_eq!(std::fs::read(&src).unwrap(), before);
    let a_bytes = std::fs::read(&a).unwrap();
    let b = path(app(
        &["convert", src.to_str().unwrap(), "-f", "mp4"],
        t.path(),
    ));
    assert_ne!(a, b);
    assert_eq!(std::fs::read(a).unwrap(), a_bytes);
}
#[test]
fn clips_frames_audio_and_images() {
    let t = tempfile::tempdir().unwrap();
    let src = fixture(t.path());
    let clip = path(app(
        &[
            "convert",
            src.to_str().unwrap(),
            "-f",
            "mp3",
            "--start",
            "0.5",
            "--end",
            "1.5",
        ],
        t.path(),
    ));
    let out = app(&["probe", clip.to_str().unwrap()], t.path());
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let duration = v["format"]["duration"]
        .as_str()
        .unwrap()
        .parse::<f64>()
        .unwrap();
    assert!((0.9..1.2).contains(&duration));
    let frames = path(app(
        &[
            "convert",
            src.to_str().unwrap(),
            "-f",
            "png",
            "--frames",
            "-q",
            "1",
        ],
        t.path(),
    ));
    assert_eq!(std::fs::read_dir(&frames).unwrap().count(), 2);
    let first = std::fs::read_dir(frames)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let jpg = path(app(
        &[
            "convert",
            first.to_str().unwrap(),
            "-f",
            "jpg",
            "--strip-exif",
            "-q",
            "50",
        ],
        t.path(),
    ));
    assert!(jpg.metadata().unwrap().len() > 0);
}
#[test]
fn invalid_media_cleans_staging() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("bad.mp4");
    std::fs::write(&p, "not a video").unwrap();
    assert!(
        !app(&["convert", p.to_str().unwrap(), "-f", "mp3"], t.path())
            .status
            .success()
    );
    assert!(std::fs::read_dir(t.path()).unwrap().all(|e| {
        !e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".gingertea-")
    }));
}
