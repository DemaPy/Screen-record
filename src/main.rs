//! recorder — record human-like browser walkthroughs of a queue of URLs (macOS).
//!
//! Flow: check prereqs (ffmpeg via brew, Chromium via fetcher) → discover the
//! AVFoundation screen device → launch Chromium → manual login → walk the URL
//! queue, recording each to its own MP4 → write report.json.

mod browser;
mod cli;
mod config;
mod console;
mod devices;
mod error;
mod ffmpeg;
mod filename;
mod login;
mod orchestrator;
mod prereqs;
mod report;
mod viewer;

use anyhow::{bail, Context, Result};
use clap::Parser;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::browser::BrowserSession;
use crate::cli::Cli;
use crate::console::ConsoleCapture;
use crate::report::Reporter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let urls = cli.resolve_urls()?;
    if urls.is_empty() {
        bail!("no URLs provided. Pass them as arguments and/or via --file <path>.");
    }
    let cfg = cli.build_config();

    std::fs::create_dir_all(&cli.output_dir)
        .with_context(|| format!("creating output dir {}", cli.output_dir.display()))?;

    // 1. Prerequisites.
    prereqs::ensure_ffmpeg().await?;
    let chrome_path = prereqs::ensure_chromium().await?;

    // 2. Screen capture device.
    let device_index = match cfg.device_index {
        Some(d) => d,
        None => devices::discover_screen_index().await?,
    };
    info!("using AVFoundation screen device index {device_index}");

    // 3. Browser + session.
    let session = BrowserSession::launch(chrome_path).await?;
    let capture = ConsoleCapture::attach(&session.page).await?;

    // 4. Manual login (optional login URL).
    login::manual_login(&session, cli.login_url.as_deref()).await?;

    // 5. Walk the queue.
    let mut reporter = Reporter::new(&cli.output_dir);
    info!("recording {} URL(s)", urls.len());
    let result = orchestrator::run_queue(
        &session,
        &capture,
        &cfg,
        device_index,
        &cli.output_dir,
        &urls,
        &mut reporter,
    )
    .await;

    // 6. Clean up and finalize the report regardless of outcome.
    session.close().await;
    reporter.flush()?;
    info!(
        "report written to {}",
        cli.output_dir.join("report.json").display()
    );

    result
}
