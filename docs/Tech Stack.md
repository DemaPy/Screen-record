---
tags: [recorder, stack, dependencies]
aliases: [Tech Stack, Stack, Dependencies]
---

# 🧱 Tech Stack

Back to [[Recorder - Home]]. *Why* each choice → [[Decisions (ADR)]].

## Language & runtime

| Thing | Choice | Why |
|---|---|---|
| Language | **Rust** (edition 2021) | User's requirement |
| Async runtime | **tokio** (`features = ["full"]`) | Process spawning, timers, async I/O for ffmpeg + CDP |
| Platform | **macOS only** | AVFoundation screen capture is macOS-specific |

## External tools (not crates)

| Tool | Role | How we get it |
|---|---|---|
| **FFmpeg** | Screen capture + HEVC encode | Auto-installed via `brew install ffmpeg` if missing — [[Module Reference#prereqs.rs]] |
| **Chromium** | The browser we film | Auto-downloaded by `chromiumoxide`'s fetcher, cached locally |
| **Homebrew** | Installs ffmpeg | Must be present; else we error with manual instructions |

## Crates (from `Cargo.toml`)

| Crate | Used for | Where |
|---|---|---|
| `tokio` | async runtime, `process`, `time`, `io` | everywhere |
| `chromiumoxide` (`fetcher`, `native-tls`, `zip8`) | drive Chromium over [[Glossary#CDP — Chrome DevTools Protocol\|CDP]]; fetch the browser | [[Module Reference#browser.rs]], [[Module Reference#prereqs.rs]] |
| `futures` | drive the CDP `Handler` stream + event listeners | [[Module Reference#browser.rs]], [[Module Reference#console.rs]] |
| `clap` (derive) | CLI parsing | [[Module Reference#cli.rs]] |
| `serde` / `serde_json` | `report.json`, console event serialization | [[Module Reference#report.rs]], [[Module Reference#console.rs]] |
| `chrono` | timestamps for filenames + report | [[Module Reference#filename.rs]] |
| `tracing` / `tracing-subscriber` | structured logging (set `RUST_LOG`) | everywhere |
| `anyhow` | app-level error propagation | per-URL flow |
| `thiserror` | typed **setup** errors | [[Module Reference#error.rs]] |
| `regex` | parse the avfoundation device list; sanitize filenames | [[Module Reference#devices.rs]], [[Module Reference#filename.rs]] |
| `dirs` | locate the OS cache dir for the Chromium download | [[Module Reference#prereqs.rs]] |
| `which` | detect `ffmpeg` / `brew` on PATH | [[Module Reference#prereqs.rs]] |
| `rand` | randomize human-like scroll/dwell | [[Module Reference#viewer.rs]] |
| `libc` | send SIGINT/SIGTERM to FFmpeg (graceful stop) | [[Module Reference#ffmpeg.rs]] |

> [!note] chromiumoxide feature flags explained
> - `fetcher` → enables the bundled Chromium downloader.
> - `native-tls` → the fetcher uses macOS's TLS to download.
> - `zip8` → the zip implementation used to unpack the downloaded browser.
> The tokio runtime is hard-wired in chromiumoxide 0.9 (no runtime feature flag).

## The FFmpeg command we build

```bash
ffmpeg -y -hide_banner -loglevel info \
  -f avfoundation -framerate 24 -capture_cursor 1 -i "1:none" \
  -vf scale=-2:480 \
  -c:v hevc_videotoolbox -b:v 1200k -tag:v hvc1 -pix_fmt yuv420p \
  -t 1200 \
  -progress pipe:1 \
  output.mp4
```

Decoded:
- `-f avfoundation … -i "1:none"` → capture screen device index 1, no audio.
- `-vf scale=-2:480` → downscale to 480p tall, width auto (keeps aspect). See [[Glossary#HEVC / H.265]].
- `-c:v hevc_videotoolbox -b:v 1200k -tag:v hvc1` → hardware HEVC at ~1.2 Mbit/s, playable in QuickTime.
- `-t 1200` → hard 20-minute cap.
- `-progress pipe:1` → readiness signal (see [[Data Flow]]).

#recorder
