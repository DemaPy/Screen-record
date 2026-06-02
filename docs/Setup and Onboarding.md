---
tags: [recorder, onboarding, setup, howto]
aliases: [Setup, Onboarding, Getting Started]
---

# 🚀 Setup and Onboarding

Back to [[Recorder - Home]]. Goal: a junior goes from zero → first recording in ~15 minutes.

## 0. Prerequisites

- **macOS** (Sonoma / Sequoia). This will not work on Linux/Windows — see [[Glossary#AVFoundation]].
- **Rust toolchain** — `cargo --version` should work. Install: <https://rustup.rs>.
- **Homebrew** — `brew --version`. Needed to auto-install ffmpeg.

> [!danger] The #1 onboarding trap: Screen Recording permission
> macOS blocks screen capture until you grant the **terminal app you run from** Screen Recording permission — **and reopen it**. Without this, FFmpeg hangs and every URL fails. Details + symptoms: [[Gotchas and Edge Cases#Black frames — Screen Recording permission]].
>
> **Grant it now:** System Settings → Privacy & Security → Screen Recording → enable your terminal (Terminal.app, iTerm, **or Cursor/VS Code if you run from their integrated terminal**) → **quit and reopen it**.

## 1. Build

```bash
git clone https://github.com/DemaPy/Screen-record.git
cd Screen-record          # the project dir
cargo build               # first build compiles deps (~1 min)
cargo test                # 6 unit tests should pass
cargo clippy              # should be clean
```

## 2. First run (single public URL — no login needed)

```bash
cargo run -- https://example.com
```

What happens (see [[Data Flow]]):
1. First ever run **downloads Chromium** (cached afterward) — be patient once.
2. A Chromium window opens and **maximizes**.
3. You see: `>>> Navigate to your site and log in, then press Enter…` — for a public URL just **press Enter**.
4. It records, scrolls, stops, and writes `recordings/2026-..._example-domain.mp4` + `report.json`.

> [!success] How to know it worked
> `recordings/report.json` shows `"succeeded": 1`, and the `.mp4` plays in QuickTime showing the page (not black). Quick check:
> ```bash
> ffprobe recordings/*.mp4      # codec=hevc, height=480
> open recordings/*.mp4         # plays in QuickTime
> ```

## 3. Real run (login + a list of URLs)

```bash
# urls.txt: one URL per line (# comments / blank lines ignored)
cargo run -- --login-url https://yoursite/login --file urls.txt
```

Now the flow is: window opens on the login page → **you log in by hand** → press Enter → it records every URL in `urls.txt` in that logged-in session. See [[Decisions (ADR)#ADR-5 Single reused page + manual login]].

> [!warning] Don't touch the machine during a run
> Because we **film the whole screen**, anything you bring to the foreground gets recorded instead of the browser. Start it and walk away. (We re-foreground Chromium per URL, but not continuously within one recording — [[Decisions (ADR)#ADR-10 Foreground via open -a]].)

## 4. Useful knobs

| Flag | Default | Purpose |
|---|---|---|
| `--file <path>` | – | read URLs from a file |
| `--login-url <url>` | – | open this page for manual login first |
| `--output-dir <dir>` | `recordings` | where MP4s/errors/report go |
| `--max-seconds <n>` | 1200 | hard cap per recording |
| `--bitrate <r>` | `1200k` | video bitrate (size vs quality) |
| `--device-index <n>` | auto | override the AVFoundation screen index |

Logging: `RUST_LOG=debug cargo run -- …` for verbose output (powered by `tracing`).
Tuning the human-like feel: edit the viewing knobs in [[Module Reference#config.rs]].

## 5. Your first contribution (orientation)

- Want to change *behavior*? Start in [[Module Reference#orchestrator.rs]] (the per-URL flow).
- Change *how it scrolls*? [[Module Reference#viewer.rs]] + knobs in [[Module Reference#config.rs]].
- Change *recording settings*? [[Module Reference#ffmpeg.rs]] + [[Tech Stack#The FFmpeg command we build]].
- Hit something weird? Check [[Gotchas and Edge Cases]] **first** — most surprises are documented there.

## 6. Troubleshooting cheat-sheet

| Symptom | Likely cause | Fix |
|---|---|---|
| All URLs fail "never produced frames" | Screen Recording permission | grant + **reopen** terminal — [[Gotchas and Edge Cases#Black frames — Screen Recording permission]] |
| Video is black | same as above | same |
| Page tiny in the video | window not maximized | [[Gotchas and Edge Cases#Window must be maximized AND frontmost]] |
| Recorded the wrong app | focus stolen mid-run | don't use the machine during a run |
| `no matches found` on a URL | shell glob (parens) | **quote** the URL — [[Gotchas and Edge Cases#Shell globbing of URLs]] |
| MP4 won't play | missing `hvc1` / hard kill | check `forced_stop` in report — [[Glossary#hvc1 tag]] |

#recorder
