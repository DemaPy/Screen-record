# Screen-record

Record human-like browser walkthroughs of a queue of URLs on **macOS**.

It logs into a site **once** (manually), then walks a queue of URLs in the same
logged-in session, recording each page to its own MP4 via FFmpeg AVFoundation
screen capture with human-like scrolling, dwell, and pauses. A `report.json`
summarizes every URL.

## Requirements

- macOS (Sonoma / Sequoia)
- Rust toolchain (`cargo`)
- FFmpeg — auto-installed via Homebrew if missing
- Chromium — auto-downloaded on first run via chromiumoxide's fetcher

### ⚠️ Screen Recording permission (required)

AVFoundation screen capture is gated by macOS privacy. **Grant your terminal app
Screen Recording permission**, then **fully quit and reopen the terminal** (the
grant only applies to a freshly launched process):

> System Settings → Privacy & Security → Screen Recording → enable your terminal

Without this, FFmpeg hangs without producing frames and the recorder reports the
URL as failed with an explanatory message.

## Usage

```bash
# Build
cargo build --release

# Record a few URLs (positional args)
cargo run --release -- https://example.com https://news.ycombinator.com

# Record URLs from a file (one per line; # comments and blanks ignored)
cargo run --release -- --file urls.example.txt

# Open a login page first; log in by hand, press Enter, then the queue runs
cargo run --release -- --login-url https://example.com/login --file urls.example.txt
```

Output (default `./recordings/`):

- `YYYY-MM-DD_HH-MM-SS_<page-title>.mp4` — one per URL
- `errors/<stamp>_<slug>.png` and `.log` — screenshot + console/ffmpeg log for failures
- `report.json` — per-URL results plus a success/failure summary

### Useful flags

| Flag | Default | Description |
|------|---------|-------------|
| `--file <path>` | – | Read URLs from a file (one per line) |
| `--login-url <url>` | – | Open this page for manual login before the queue |
| `--output-dir <dir>` | `recordings` | Where MP4s, errors, and report.json go |
| `--device-index <n>` | auto | AVFoundation screen device index |
| `--framerate <n>` | 24 | Capture framerate |
| `--bitrate <r>` | `1200k` | Target video bitrate |
| `--max-seconds <n>` | 1200 | Hard cap per recording (20 min) |

## Encoding

Full screen, downscaled to 480p (`scale=-2:480`, aspect preserved), encoded with
the Mac hardware HEVC encoder (`hevc_videotoolbox`, tagged `hvc1` so QuickTime/
Safari play it). Small files and low CPU for long captures of a 4K display.

## Documentation

A full **Obsidian-style docs vault** lives in [`docs/`](docs/) — open that folder
as an Obsidian vault for linked navigation and diagrams. Start at
`docs/Recorder - Home.md`:

- **Setup and Onboarding** — get running in ~15 min (read this first)
- **Architecture** — modules and how they fit
- **Data Flow** — what happens, step by step
- **Tech Stack** — every crate/tool and why
- **Decisions (ADR)** — the non-obvious design choices
- **Module Reference** — file-by-file code map
- **Gotchas and Edge Cases** — the traps (Screen Recording permission, etc.)
- **Glossary** — the core terms
