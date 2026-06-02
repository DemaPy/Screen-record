//! Runtime configuration for the recorder. Defaults match the agreed plan:
//! full-screen capture downscaled to 480p, encoded with the Mac hardware HEVC
//! encoder, with a 20-minute hard cap and human-like viewing knobs.

use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RecorderConfig {
    // --- FFmpeg / capture ---
    /// AVFoundation video device index. `None` => auto-discover the screen.
    pub device_index: Option<u32>,
    pub framerate: u32,
    /// Output height; width is auto (`scale=-2:<h>`) to preserve aspect ratio.
    pub scale_height: u32,
    /// FFmpeg video encoder (Mac hardware HEVC by default).
    pub video_encoder: String,
    /// Target bitrate for the hardware encoder (it does not use CRF).
    pub bitrate: String,
    pub pix_fmt: String,
    /// Hard cap on a single recording (safety against infinite-scroll pages).
    pub max_record: Duration,

    // --- Per-URL timing ---
    /// Pause after bringing the window to front, before starting capture.
    pub pre_record_wait: Duration,
    /// Fallback wait for AVFoundation to start producing frames if the
    /// "capture started" stderr line is not seen.
    pub capture_ready_fallback: Duration,

    // --- Human-like viewing ---
    /// Pixels per scroll step (randomized within this inclusive range).
    pub scroll_step_px: (i64, i64),
    /// Delay between scroll steps (randomized within this inclusive range).
    pub scroll_delay: (Duration, Duration),
    /// Dwell after scrolling roughly one viewport (randomized).
    pub section_dwell: (Duration, Duration),
    /// Probability (0..=100) of a short pause between scroll segments.
    pub pause_chance_pct: u8,
    /// Probability (0..=100) of an occasional scroll-up re-read.
    pub reread_chance_pct: u8,
    /// Max times we allow the page to grow (infinite-scroll guard).
    pub max_infinite_extensions: u32,
    /// How long to wait for `scrollHeight` to settle (lazy-load detection).
    pub lazy_settle: Duration,
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            device_index: None,
            framerate: 24,
            scale_height: 480,
            video_encoder: "hevc_videotoolbox".to_string(),
            bitrate: "1200k".to_string(),
            pix_fmt: "yuv420p".to_string(),
            max_record: Duration::from_secs(20 * 60),

            pre_record_wait: Duration::from_secs(2),
            capture_ready_fallback: Duration::from_millis(1500),

            scroll_step_px: (40, 120),
            scroll_delay: (Duration::from_millis(60), Duration::from_millis(180)),
            section_dwell: (Duration::from_millis(1200), Duration::from_millis(3500)),
            pause_chance_pct: 25,
            reread_chance_pct: 15,
            max_infinite_extensions: 8,
            lazy_settle: Duration::from_millis(1200),
        }
    }
}
