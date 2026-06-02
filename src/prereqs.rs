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

/// Locations of a real, user-installed Chrome-family browser, in preference
/// order. Real Google Chrome is far less likely to be flagged by bot detection
/// than a fetched Chromium (genuine "Google Chrome" UA brand, proprietary
/// codecs, Widevine), so we use it when present and fall back to the fetcher.
const INSTALLED_CHROME_PATHS: &[&str] = &[
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Google Chrome Beta.app/Contents/MacOS/Google Chrome Beta",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
];

/// Return the path to an installed Chrome-family browser, if one exists.
pub fn find_installed_chrome() -> Option<PathBuf> {
    INSTALLED_CHROME_PATHS
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
}

/// Is a personal Chrome-family browser already running? If so we must NOT reuse
/// its `.app` bundle: `bring_to_front` raises the app by bundle (`open -a`), and
/// sharing the bundle with the user's everyday browser would foreground — and
/// thus film — their personal windows. The fetched Chromium has a *unique*
/// bundle, so it's always safe.
fn personal_chrome_running() -> bool {
    INSTALLED_CHROME_PATHS
        .iter()
        .filter_map(|p| std::path::Path::new(p).file_name())
        .any(|name| {
            std::process::Command::new("pgrep")
                .arg("-x")
                .arg(name)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        })
}

/// Resolve a browser executable: prefer an installed Chrome (better against bot
/// detection), otherwise download/reuse a cached Chromium via the fetcher.
///
/// We only use installed Chrome when none is *already running* — otherwise the
/// shared `.app` bundle would let `bring_to_front` raise the user's personal
/// window into the recording (see [`personal_chrome_running`]).
pub async fn ensure_browser() -> Result<PathBuf, SetupError> {
    match find_installed_chrome() {
        Some(chrome) if !personal_chrome_running() => {
            info!("using installed Chrome at {}", chrome.display());
            Ok(chrome)
        }
        Some(_) => {
            warn!(
                "a personal Chrome is already running; using fetched Chromium instead so we don't \
                 foreground your personal windows. Quit Chrome to use it (better against bot detection)."
            );
            ensure_chromium().await
        }
        None => {
            info!("no installed Chrome found; falling back to fetched Chromium");
            ensure_chromium().await
        }
    }
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
