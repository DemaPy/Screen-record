//! Startup prerequisite checks. We auto-install ffmpeg via Homebrew if missing,
//! and download a pinned Chromium via chromiumoxide's fetcher into a local cache.

use std::path::PathBuf;

use chromiumoxide::fetcher::{BrowserFetcher, BrowserFetcherOptions};
use tokio::process::Command;
use tracing::{info, warn};

use crate::error::SetupError;

/// Ensure `ffmpeg` is on PATH; if not, try `brew install ffmpeg`.
pub async fn ensure_ffmpeg() -> Result<(), SetupError> {
    if which::which("ffmpeg").is_ok() {
        info!("ffmpeg found on PATH");
        return Ok(());
    }
    warn!("ffmpeg not found; attempting `brew install ffmpeg`");

    if which::which("brew").is_err() {
        return Err(SetupError::BrewMissing);
    }

    let status = Command::new("brew")
        .args(["install", "ffmpeg"])
        .status()
        .await
        .map_err(|e| SetupError::FfmpegMissing(format!("could not run brew: {e}")))?;

    if !status.success() {
        return Err(SetupError::FfmpegMissing(format!(
            "`brew install ffmpeg` exited with {status}"
        )));
    }
    if which::which("ffmpeg").is_err() {
        return Err(SetupError::FfmpegMissing(
            "brew reported success but ffmpeg still not on PATH".to_string(),
        ));
    }
    info!("ffmpeg installed via Homebrew");
    Ok(())
}

/// Download (or reuse a cached) Chromium and return the executable path.
pub async fn ensure_chromium() -> Result<PathBuf, SetupError> {
    let cache = dirs::cache_dir()
        .ok_or_else(|| SetupError::Chromium("no OS cache directory available".to_string()))?
        .join("recorder")
        .join("chromium");

    std::fs::create_dir_all(&cache)
        .map_err(|e| SetupError::Chromium(format!("creating cache dir {cache:?}: {e}")))?;

    info!("ensuring Chromium is available in {}", cache.display());

    let options = BrowserFetcherOptions::builder()
        .with_path(&cache)
        .build()
        .map_err(|e| SetupError::Chromium(format!("building fetcher options: {e}")))?;

    let fetcher = BrowserFetcher::new(options);
    let info = fetcher
        .fetch()
        .await
        .map_err(|e| SetupError::Chromium(format!("fetching Chromium: {e}")))?;

    info!("using Chromium at {}", info.executable_path.display());
    Ok(info.executable_path)
}
