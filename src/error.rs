//! Error types. Per-URL failures are recoverable (recorded in the report and
//! skipped); setup failures (missing ffmpeg, no screen device, browser launch)
//! abort the run.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SetupError {
    #[error("ffmpeg is required but not installed, and automatic install failed: {0}")]
    FfmpegMissing(String),
    #[error("Homebrew is required to auto-install ffmpeg but was not found. Install ffmpeg manually: https://ffmpeg.org/download.html")]
    BrewMissing,
    #[error("could not download/locate Chromium: {0}")]
    Chromium(String),
    #[error("no AVFoundation screen-capture device found. Output of list_devices:\n{0}")]
    NoScreenDevice(String),
    #[error("failed to launch browser: {0}")]
    BrowserLaunch(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
