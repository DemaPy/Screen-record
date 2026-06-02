//! Human-like page viewing. Driven from Rust with many small `scrollBy` steps
//! (the small steps + randomized delays produce a smooth, organic scroll),
//! interleaved with section dwells, occasional pauses, and occasional scroll-up
//! re-reads. Handles lazy-load (wait for height to settle) and infinite-scroll
//! (capped by extension count and an overall time budget).

use std::time::{Duration, Instant};

use anyhow::Result;
use chromiumoxide::page::Page;
use rand::Rng;
use serde::Deserialize;
use tracing::debug;

use crate::config::RecorderConfig;

#[derive(Debug, Deserialize)]
struct Metrics {
    /// Current vertical scroll offset.
    y: f64,
    /// Maximum scrollable offset (scrollHeight - innerHeight).
    max: f64,
    /// Viewport height.
    h: f64,
}

async fn read_metrics(page: &Page) -> Result<Metrics> {
    let res = page
        .evaluate(
            "(() => ({ \
                y: window.scrollY, \
                max: Math.max(0, document.documentElement.scrollHeight - window.innerHeight), \
                h: window.innerHeight \
            }))()",
        )
        .await?;
    Ok(res.into_value()?)
}

fn rand_dur(rng: &mut impl Rng, range: (Duration, Duration)) -> Duration {
    let lo = range.0.as_millis() as u64;
    let hi = range.1.as_millis() as u64;
    Duration::from_millis(rng.gen_range(lo..=hi.max(lo)))
}

/// Scroll through and "read" the page like a human. Returns when the bottom is
/// reached (with no further lazy growth), the extension cap is hit, or the time
/// budget is exhausted.
pub async fn view_page(page: &Page, cfg: &RecorderConfig) -> Result<()> {
    let deadline = Instant::now() + cfg.max_record;
    let mut rng = rand::thread_rng();
    let mut extensions = 0u32;

    // Let above-the-fold content settle and be "read" first.
    tokio::time::sleep(rand_dur(&mut rng, cfg.section_dwell)).await;

    loop {
        if Instant::now() >= deadline {
            debug!("viewer hit time budget");
            break;
        }

        let m = read_metrics(page).await?;

        // At/near the bottom: check for lazy-load / infinite-scroll growth.
        if m.y + 2.0 >= m.max {
            tokio::time::sleep(cfg.lazy_settle).await;
            let after = read_metrics(page).await?;
            if after.max > m.max + 4.0 && extensions < cfg.max_infinite_extensions {
                extensions += 1;
                debug!("page grew (extension {extensions}); continuing");
                continue;
            }
            debug!("reached bottom; viewing complete");
            break;
        }

        // Scroll roughly one viewport in small randomized steps.
        let segment = m.h * rng.gen_range(0.6..1.0);
        let mut scrolled = 0.0;
        while scrolled < segment {
            if Instant::now() >= deadline {
                break;
            }
            let step = rng.gen_range(cfg.scroll_step_px.0..=cfg.scroll_step_px.1);
            let _ = page.evaluate(format!("window.scrollBy(0,{step})")).await;
            scrolled += step as f64;
            tokio::time::sleep(rand_dur(&mut rng, cfg.scroll_delay)).await;
        }

        // Dwell on the section we just scrolled to.
        tokio::time::sleep(rand_dur(&mut rng, cfg.section_dwell)).await;

        // Occasional longer pause.
        if rng.gen_range(0..100) < cfg.pause_chance_pct {
            tokio::time::sleep(rand_dur(&mut rng, cfg.section_dwell)).await;
        }

        // Occasional scroll-up re-read, then continue downward.
        if rng.gen_range(0..100) < cfg.reread_chance_pct {
            let up = rng.gen_range(cfg.scroll_step_px.1..=cfg.scroll_step_px.1 * 3);
            let _ = page.evaluate(format!("window.scrollBy(0,-{up})")).await;
            tokio::time::sleep(rand_dur(&mut rng, cfg.section_dwell)).await;
        }
    }

    Ok(())
}
