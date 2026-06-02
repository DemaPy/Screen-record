//! Command-line interface. URLs come from positional args and/or a `--file`
//! (one URL per line, `#` comments and blank lines ignored). A few recorder
//! defaults can be overridden here.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use crate::config::RecorderConfig;

#[derive(Parser, Debug)]
#[command(
    name = "recorder",
    about = "Record human-like browser walkthroughs of a queue of URLs (macOS).",
    long_about = "Logs into a site once (manual), then records each queued URL to its own MP4 \
using FFmpeg AVFoundation screen capture with human-like scrolling."
)]
pub struct Cli {
    /// URLs to record (in order). Combined with any read from --file.
    pub urls: Vec<String>,

    /// Path to a file with one URL per line (# comments and blanks ignored).
    #[arg(short, long)]
    pub file: Option<PathBuf>,

    /// Optional login page opened first; you log in by hand, then press Enter.
    #[arg(long)]
    pub login_url: Option<String>,

    /// Directory for output MP4s, error artifacts, and report.json.
    #[arg(long, default_value = "recordings")]
    pub output_dir: PathBuf,

    /// Override AVFoundation device index (default: auto-detect screen).
    #[arg(long)]
    pub device_index: Option<u32>,

    /// Override capture framerate.
    #[arg(long)]
    pub framerate: Option<u32>,

    /// Override target bitrate (e.g. "1500k").
    #[arg(long)]
    pub bitrate: Option<String>,

    /// Override the per-recording hard cap, in seconds (default 1200 = 20 min).
    #[arg(long)]
    pub max_seconds: Option<u64>,
}

impl Cli {
    /// Resolve the full ordered URL list from positional args plus --file.
    pub fn resolve_urls(&self) -> Result<Vec<String>> {
        let mut urls: Vec<String> = self.urls.clone();
        if let Some(path) = &self.file {
            let contents = std::fs::read_to_string(path)
                .with_context(|| format!("reading url file {}", path.display()))?;
            for line in contents.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                urls.push(line.to_string());
            }
        }
        Ok(urls)
    }

    /// Apply CLI overrides on top of the default recorder config.
    pub fn build_config(&self) -> RecorderConfig {
        let mut cfg = RecorderConfig::default();
        if let Some(d) = self.device_index {
            cfg.device_index = Some(d);
        }
        if let Some(f) = self.framerate {
            cfg.framerate = f;
        }
        if let Some(b) = &self.bitrate {
            cfg.bitrate = b.clone();
        }
        if let Some(s) = self.max_seconds {
            cfg.max_record = std::time::Duration::from_secs(s);
        }
        cfg
    }
}
