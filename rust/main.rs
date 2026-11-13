mod banner;
mod download;
mod media;
mod motion;
mod state;
mod terminal_colors;
mod text_input;
mod tools;
mod ui;
use anyhow::{Result, bail};
use clap::{Args, Parser, Subcommand};
use media::{Cancel, Choice, Reporter};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
#[derive(Parser)]
#[command(
    name = "gingertea",
    version,
    about = "Download and convert media in your terminal"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    #[arg(value_name = "URL_OR_FILE")]
    input: Option<String>,
}
#[derive(Subcommand)]
enum Commands {
    Get {
        url: String,
        #[command(flatten)]
        options: Options,
        #[arg(long)]
        audio: bool,
        #[arg(long)]
        dir: Option<String>,
    },
    Convert {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        #[command(flatten)]
        options: Options,
        #[arg(long)]
        frames: bool,
    },
    Setup,
    Update,
    Probe {
        input: String,
    },
    Sites {
        query: Option<String>,
    },
}
#[derive(Args, Clone, Default)]
struct Options {
    #[arg(short = 'f', long = "format")]
    fmt: Option<String>,
    #[arg(short = 'q', long, default_value = "best")]
    quality: String,
    #[arg(long)]
    start: Option<String>,
    #[arg(long)]
    end: Option<String>,
    #[arg(long)]
    subs: Option<String>,
    #[arg(long)]
    embed_subs: bool,
    #[arg(long)]
    srt: bool,
    #[arg(long,value_parser=["off","cut","chapters"])]
    sponsorblock: Option<String>,
    #[arg(long)]
    chapters: bool,
    #[arg(long)]
    loudnorm: bool,
    #[arg(long)]
    max_kb: Option<u64>,
    #[arg(long)]
    strip_exif: bool,
}
impl Options {
    fn choice(&self, kind: &str, default_fmt: &str) -> Result<Choice> {
        let c = Choice {
            kind: kind.into(),
            fmt: self.fmt.clone().unwrap_or(default_fmt.into()),
            quality: self.quality.clone(),
            start: state::parse_time(self.start.as_deref().unwrap_or(""))?,
            end: state::parse_time(self.end.as_deref().unwrap_or(""))?,
            subtitles: if self.embed_subs {
                "embed"
            } else if self.srt || self.subs.is_some() {
                "srt"
            } else {
                "off"
            }
            .into(),
            languages: self.subs.clone().unwrap_or_default(),
            sponsorblock: self.sponsorblock.clone().unwrap_or("off".into()),
            chapters: self.chapters,
            loudnorm: self.loudnorm,
            max_kb: self.max_kb,
            strip_exif: self.strip_exif,
        };
        c.validate()?;
        Ok(c)
    }
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e:#}");
        std::process::exit(1)
    }
}
fn run() -> Result<()> {
    let cli = Cli::parse();
    let cancel: Cancel = Arc::new(AtomicBool::new(false));
    let c = cancel.clone();
    ctrlc::set_handler(move || {
        c.store(true, Ordering::Relaxed);
        media::shutdown_workers();
    })?;
    let report: Reporter = Arc::new(|s| {
        if !s.is_empty() {
            eprintln!("{s}")
        }
    });
    let mut settings = state::Settings::load();
    match cli.command {
        Some(Commands::Get {
            url,
            options,
            audio,
            dir,
        }) => {
            if let Some(d) = dir {
                settings.download_dir = d
            }
            let mut choice = options.choice(
                if audio { "audio" } else { "video" },
                if audio { "mp3" } else { "mp4" },
            )?;
            if choice.subtitles == "off" {
                choice.subtitles = settings.subtitle_mode.clone();
                choice.languages = settings.subtitle_langs.clone();
            }
            if options.sponsorblock.is_none() {
                choice.sponsorblock = settings.sponsorblock.clone();
            }
            choice.chapters |= settings.split_chapters;
            choice.loudnorm |= settings.loudnorm;
            for path in download::download(&url, &choice, &settings, &cancel, &report)? {
                println!("{}", path.display());
            }
        }
        Some(Commands::Convert {
            files,
            options,
            frames,
        }) => {
            let fmt = options
                .fmt
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("--format is required"))?;
            let kind = if frames {
                "frames"
            } else if media::formats("video").contains(&fmt.into()) {
                "video"
            } else if media::formats("audio").contains(&fmt.into()) {
                "audio"
            } else if media::formats("image").contains(&fmt.into()) {
                "image"
            } else {
                bail!("Unknown format: {fmt}")
            };
            let c = options.choice(kind, fmt)?;
            for path in files {
                let p = state::expand(&path.to_string_lossy());
                let info = media::probe(&p)?;
                let out = media::convert(&info, &c, &cancel, &report)?;
                println!("{}", out.display());
            }
        }
        Some(Commands::Setup) => tools::setup()?,
        Some(Commands::Update) => download::update(&cancel, &report)?,
        Some(Commands::Probe { input }) => {
            let p = state::expand(&input);
            let v = if p.is_file() {
                media::probe(&p)?.raw
            } else {
                download::probe(&input, &settings)?
            };
            println!("{}", serde_json::to_string_pretty(&v)?);
        }
        Some(Commands::Sites { query }) => {
            for site in download::sites()? {
                if query
                    .as_ref()
                    .is_none_or(|q| site.to_lowercase().contains(&q.to_lowercase()))
                {
                    println!("{site}")
                }
            }
        }
        None => ui::run(settings, cli.input.unwrap_or_default(), cancel)?,
    }
    Ok(())
}
