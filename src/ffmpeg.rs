//! FFmpeg AVFoundation screen recorder.
//!
//! Lifecycle per URL: [`FfmpegRecorder::start`] spawns ffmpeg, then
//! [`FfmpegRecorder::wait_until_ready`] blocks until frames are actually
//! flowing (AVFoundation has start-up latency), and [`FfmpegRecorder::stop`]
//! shuts it down *gracefully* so the MP4's moov atom is finalized and the file
//! is playable. Graceful escalation is `q` → SIGINT → SIGTERM → SIGKILL; only
//! the last resort (SIGKILL) corrupts the file.

use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::{debug, warn};

use crate::config::RecorderConfig;

pub struct FfmpegRecorder {
    child: Child,
    stdin: Option<ChildStdin>,
    ready_rx: Option<oneshot::Receiver<()>>,
    reader: Option<JoinHandle<()>>,
    log: Arc<Mutex<String>>,
}

impl FfmpegRecorder {
    /// Spawn ffmpeg capturing `device_index` to `output_path`.
    pub fn start(
        device_index: u32,
        output_path: &Path,
        cfg: &RecorderConfig,
    ) -> Result<Self> {
        let mut cmd = Command::new("ffmpeg");
        cmd.arg("-y")
            .arg("-hide_banner")
            .args(["-loglevel", "info"])
            // --- input: AVFoundation screen capture ---
            .args(["-f", "avfoundation"])
            .args(["-framerate", &cfg.framerate.to_string()])
            .args(["-capture_cursor", "1"])
            .args(["-i", &format!("{device_index}:none")])
            // --- output: full screen downscaled to 480p, hardware HEVC ---
            .args(["-vf", &format!("scale=-2:{}", cfg.scale_height)])
            .args(["-c:v", &cfg.video_encoder])
            .args(["-b:v", &cfg.bitrate]);

        if cfg.video_encoder.contains("hevc") {
            // Make the HEVC stream play in QuickTime/Safari.
            cmd.args(["-tag:v", "hvc1"]);
        }

        cmd.args(["-pix_fmt", &cfg.pix_fmt])
            // Hard safety cap so a hung/infinite page can't exceed the limit.
            .args(["-t", &cfg.max_record.as_secs().to_string()])
            // Machine-readable progress to stdout: newline-delimited key=value
            // blocks. Unlike the stderr stats line (which uses '\r'), these are
            // reliable to parse for capture readiness.
            .args(["-progress", "pipe:1"])
            .arg(output_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = cmd.spawn().context("spawning ffmpeg")?;

        let stdin = child.stdin.take();
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("no ffmpeg stdout"))?;
        let stderr = child.stderr.take().ok_or_else(|| anyhow!("no ffmpeg stderr"))?;

        let log = Arc::new(Mutex::new(String::new()));
        let (ready_tx, ready_rx) = oneshot::channel();

        // Readiness: parse the -progress stream on stdout. The first `frame=N`
        // with N >= 1 means AVFoundation is actually producing frames.
        let progress_reader = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            let mut ready_tx = Some(ready_tx);
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(rest) = line.strip_prefix("frame=") {
                    if rest.trim().parse::<u64>().map(|n| n >= 1).unwrap_or(false) {
                        if let Some(tx) = ready_tx.take() {
                            let _ = tx.send(());
                        }
                    }
                }
            }
        });

        // Drain stderr continuously into the log buffer (used by the report).
        let log_clone = Arc::clone(&log);
        let reader = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(mut buf) = log_clone.lock() {
                    buf.push_str(&line);
                    buf.push('\n');
                }
            }
            // Keep the progress reader's lifetime tied here so it is not dropped
            // early; awaiting it ensures the readiness signal can still fire.
            let _ = progress_reader.await;
        });

        Ok(Self {
            child,
            stdin,
            ready_rx: Some(ready_rx),
            reader: Some(reader),
            log,
        })
    }

    /// Wait until ffmpeg is actually producing frames. Call this *before*
    /// navigating so the page load is on camera. Returns `true` if frames were
    /// confirmed flowing; `false` means capture never started — almost always
    /// the macOS Screen Recording permission gate (AVFoundation blocks the
    /// device open and ffmpeg hangs without producing frames).
    pub async fn wait_until_ready(&mut self, cfg: &RecorderConfig) -> bool {
        if let Some(rx) = self.ready_rx.take() {
            match tokio::time::timeout(Duration::from_secs(8), rx).await {
                Ok(Ok(())) => {
                    debug!("ffmpeg capture is producing frames");
                    return true;
                }
                _ => {
                    warn!("did not see ffmpeg 'frame=' marker; capture may not have started");
                }
            }
        }
        tokio::time::sleep(cfg.capture_ready_fallback).await;
        false
    }

    /// Gracefully stop ffmpeg and return its captured stderr log. Returns
    /// `false` if we had to SIGKILL (the resulting MP4 may be truncated).
    pub async fn stop(mut self) -> (String, bool) {
        // 1. Ask ffmpeg to finish writing: 'q' on stdin (cleanest path).
        if let Some(mut stdin) = self.stdin.take() {
            let _ = stdin.write_all(b"q").await;
            let _ = stdin.flush().await;
            // Dropping stdin closes the pipe, nudging ffmpeg to exit.
            drop(stdin);
        }

        let clean = self.wait_with_escalation().await;

        // Ensure the stderr reader task is done before we read the log.
        if let Some(reader) = self.reader.take() {
            let _ = reader.await;
        }
        let log = self.log.lock().map(|b| b.clone()).unwrap_or_default();
        (log, clean)
    }

    /// Wait for exit, escalating q → SIGINT → SIGTERM → SIGKILL. Returns whether
    /// the process exited before we had to SIGKILL it.
    async fn wait_with_escalation(&mut self) -> bool {
        // After 'q': give it a moment to finalize the file.
        if self.wait_for(Duration::from_secs(8)).await {
            return true;
        }
        warn!("ffmpeg did not exit after 'q'; sending SIGINT");
        self.signal(libc::SIGINT);
        if self.wait_for(Duration::from_secs(5)).await {
            return true;
        }
        warn!("ffmpeg did not exit after SIGINT; sending SIGTERM");
        self.signal(libc::SIGTERM);
        if self.wait_for(Duration::from_secs(5)).await {
            return true;
        }
        warn!("ffmpeg did not exit after SIGTERM; sending SIGKILL (file may be truncated)");
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
        false
    }

    /// Wait up to `dur` for the child to exit. Returns true if it exited.
    async fn wait_for(&mut self, dur: Duration) -> bool {
        matches!(tokio::time::timeout(dur, self.child.wait()).await, Ok(Ok(_)))
    }

    /// Send a Unix signal to the ffmpeg process.
    fn signal(&self, sig: i32) {
        if let Some(pid) = self.child.id() {
            // SAFETY: pid is the live child's PID; kill() with a valid signal
            // is sound. A race where the child already exited just yields ESRCH.
            unsafe {
                libc::kill(pid as i32, sig);
            }
        }
    }
}
