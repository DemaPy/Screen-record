//! Per-URL orchestration: the sequential queue loop. Each URL is recorded
//! independently; a failure is captured (screenshot + console log) and the run
//! continues to the next URL.

use std::path::Path;
use std::time::Instant;

use anyhow::Result;
use chromiumoxide::page::ScreenshotParams;
use chrono::Local;
use tracing::{error, info};

use crate::browser::BrowserSession;
use crate::config::RecorderConfig;
use crate::console::ConsoleCapture;
use crate::ffmpeg::FfmpegRecorder;
use crate::filename::{build_filename, slug_from_url, temp_filename};
use crate::report::{Reporter, Status, UrlResult};
use crate::viewer::view_page;

pub async fn run_queue(
    session: &BrowserSession,
    console: &ConsoleCapture,
    cfg: &RecorderConfig,
    device_index: u32,
    output_dir: &Path,
    urls: &[String],
    reporter: &mut Reporter,
) -> Result<()> {
    let errors_dir = output_dir.join("errors");
    std::fs::create_dir_all(&errors_dir)?;

    let total = urls.len();
    for (i, url) in urls.iter().enumerate() {
        info!("[{}/{}] recording {url}", i + 1, total);
        let result = record_one(session, console, cfg, device_index, output_dir, &errors_dir, url)
            .await;
        match &result.status {
            Status::Success => info!(
                "[{}/{}] done -> {}",
                i + 1,
                total,
                result.output_file.as_deref().unwrap_or("?")
            ),
            Status::Failed => error!(
                "[{}/{}] failed: {}",
                i + 1,
                total,
                result.error.as_deref().unwrap_or("unknown")
            ),
        }
        reporter.record(result)?;
    }
    Ok(())
}

/// Record a single URL. Always returns a [`UrlResult`]; never propagates a
/// per-URL error (that's the failure-recovery contract).
async fn record_one(
    session: &BrowserSession,
    console: &ConsoleCapture,
    cfg: &RecorderConfig,
    device_index: u32,
    output_dir: &Path,
    errors_dir: &Path,
    url: &str,
) -> UrlResult {
    let started = Local::now();
    let t0 = Instant::now();
    console.reset();

    // 1-2: foreground + warm-up wait.
    session.bring_to_front().await;
    tokio::time::sleep(cfg.pre_record_wait).await;

    // 3: start recording to a temp name (title unknown yet).
    let temp_name = temp_filename(started, url);
    let temp_path = output_dir.join(&temp_name);

    let mut rec = match FfmpegRecorder::start(device_index, &temp_path, cfg) {
        Ok(r) => r,
        Err(e) => {
            return failure(
                session, console, errors_dir, url, started, t0, None, false,
                format!("failed to start ffmpeg: {e}"),
            )
            .await;
        }
    };

    // 4: wait for capture to actually produce frames before navigating.
    // If it never starts, it's almost always the macOS Screen Recording
    // permission gate — fail fast with an actionable message rather than
    // producing a black/empty file.
    if !rec.wait_until_ready(cfg).await {
        let (ff_log, clean) = rec.stop().await;
        let _ = std::fs::remove_file(&temp_path);
        return failure(
            session, console, errors_dir, url, started, t0, Some(ff_log), !clean,
            "ffmpeg never produced frames — capture did not start. Grant the terminal \
             Screen Recording permission (System Settings → Privacy & Security → Screen \
             Recording) and reopen the terminal."
                .to_string(),
        )
        .await;
    }

    // 5-6: navigate, read title, perform human-like viewing.
    let nav: Result<Option<String>> = async {
        session.page.goto(url).await?;
        let _ = session.page.wait_for_navigation().await;
        let title = session.page.get_title().await?;
        view_page(&session.page, cfg).await?;
        Ok(title)
    }
    .await;

    // 7: always stop ffmpeg (graceful) and collect its log.
    let (ff_log, clean) = rec.stop().await;

    match nav {
        Ok(title) => {
            // 8: rename temp -> final, derived from the page title.
            let final_name = build_filename(started, title.as_deref().unwrap_or(""), url);
            let final_path = output_dir.join(&final_name);
            if let Err(e) = std::fs::rename(&temp_path, &final_path) {
                error!("could not rename {temp_name} -> {final_name}: {e}");
            }
            UrlResult {
                url: url.to_string(),
                title,
                output_file: Some(final_name),
                status: Status::Success,
                started_at: started.to_rfc3339(),
                ended_at: Local::now().to_rfc3339(),
                duration_secs: t0.elapsed().as_secs_f64(),
                error: None,
                screenshot_path: None,
                console_log_path: None,
                forced_stop: !clean,
            }
        }
        Err(e) => {
            // Drop the partial recording; keep diagnostics instead.
            let _ = std::fs::remove_file(&temp_path);
            failure(
                session,
                console,
                errors_dir,
                url,
                started,
                t0,
                Some(ff_log),
                !clean,
                e.to_string(),
            )
            .await
        }
    }
}

/// Failure recovery: capture a screenshot and console logs, then build a failed
/// [`UrlResult`].
#[allow(clippy::too_many_arguments)]
async fn failure(
    session: &BrowserSession,
    console: &ConsoleCapture,
    errors_dir: &Path,
    url: &str,
    started: chrono::DateTime<Local>,
    t0: Instant,
    ffmpeg_log: Option<String>,
    forced_stop: bool,
    error_msg: String,
) -> UrlResult {
    let stem = format!("{}_{}", started.format("%Y-%m-%d_%H-%M-%S"), slug_from_url(url));

    // Screenshot (best-effort).
    let screenshot_path = match session
        .page
        .screenshot(ScreenshotParams::builder().build())
        .await
    {
        Ok(bytes) => {
            let p = errors_dir.join(format!("{stem}.png"));
            if std::fs::write(&p, bytes).is_ok() {
                Some(p.to_string_lossy().to_string())
            } else {
                None
            }
        }
        Err(e) => {
            error!("screenshot failed for {url}: {e}");
            None
        }
    };

    // Console + ffmpeg logs (best-effort).
    let mut log_body = console.snapshot().join("\n");
    if let Some(ff) = ffmpeg_log {
        log_body.push_str("\n\n=== ffmpeg stderr ===\n");
        log_body.push_str(&ff);
    }
    let log_path = errors_dir.join(format!("{stem}.log"));
    let console_log_path = if std::fs::write(&log_path, log_body).is_ok() {
        Some(log_path.to_string_lossy().to_string())
    } else {
        None
    };

    UrlResult {
        url: url.to_string(),
        title: None,
        output_file: None,
        status: Status::Failed,
        started_at: started.to_rfc3339(),
        ended_at: Local::now().to_rfc3339(),
        duration_secs: t0.elapsed().as_secs_f64(),
        error: Some(error_msg),
        screenshot_path,
        console_log_path,
        forced_stop,
    }
}
