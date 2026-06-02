---
tags: [recorder, reference, code]
aliases: [Module Reference, Code Map, Modules]
---

# 🗂️ Module Reference

Back to [[Recorder - Home]]. File-by-file map of `src/`. Pairs with [[Architecture]] and [[Data Flow]].

> [!tip] Reading order for code
> `main.rs` → `orchestrator.rs` → `ffmpeg.rs` → `browser.rs` → `viewer.rs`. The rest are leaves.

---

## main.rs
**Role:** entry point; wires the phases together.
**Flow:** init logging → parse [[Module Reference#cli.rs|Cli]] → resolve URLs → build [[Module Reference#config.rs|RecorderConfig]] → `prereqs::ensure_ffmpeg` + `ensure_chromium` → `devices::discover_screen_index` → `BrowserSession::launch` → `ConsoleCapture::attach` → `login::manual_login` → `orchestrator::run_queue` → `session.close()` → final report.
**Returns** the queue result; setup failures (`SetupError`) abort early.

## cli.rs
**Role:** CLI definition (`clap` derive) + URL resolution.
**Key:** `Cli` struct (positional `urls`, `--file`, `--login-url`, `--output-dir`, `--device-index`, `--framerate`, `--bitrate`, `--max-seconds`).
- `resolve_urls()` → positional + file lines (trim, skip blank/`#`).
- `build_config()` → defaults overlaid with CLI overrides.

## config.rs
**Role:** the single tunable struct.
**Key:** `RecorderConfig` + `Default`. Groups: capture (`framerate=24`, `scale_height=480`, `video_encoder=hevc_videotoolbox`, `bitrate=1200k`, `pix_fmt`, `max_record=20min`), timing (`pre_record_wait=2s`, `capture_ready_fallback`), viewing knobs (`scroll_step_px`, `scroll_delay`, `section_dwell`, `pause_chance_pct`, `reread_chance_pct`, `max_infinite_extensions`, `lazy_settle`).
**To tune the "human feel":** edit the viewing knobs here — see [[Module Reference#viewer.rs]].

## prereqs.rs
**Role:** make sure tools exist before we start.
**Key:**
- `ensure_ffmpeg()` → `which ffmpeg`; else `brew install ffmpeg`; else `SetupError::BrewMissing`.
- `ensure_chromium()` → `chromiumoxide` `BrowserFetcher` downloads a pinned Chromium into `dirs::cache_dir()/recorder/chromium`; returns the executable path. Idempotent (cached).
**Related:** [[Decisions (ADR)#ADR-12 Auto-install prerequisites]].

## devices.rs
**Role:** find the AVFoundation screen device.
**Key:** `discover_screen_index()` runs `ffmpeg -f avfoundation -list_devices true -i ""`, parses **stderr** for `[N] Capture screen` (stops before the audio section). Override via `--device-index`. Has unit tests.
**Related:** [[Glossary#AVFoundation]].

## ffmpeg.rs
**Role:** the screen-recorder service (owns the ffmpeg child process).
**Key type:** `FfmpegRecorder`.
- `start(device_index, output_path, cfg)` → spawns ffmpeg (see command in [[Tech Stack#The FFmpeg command we build]]); pipes stdin/stdout/stderr; spawns a **stdout reader** (parses `-progress` for readiness) and a **stderr reader** (log buffer).
- `wait_until_ready(cfg)` → `true` when `frame >= 1` seen (frames flowing); `false` = capture never started (usually permission). See [[Decisions (ADR)#ADR-11 Capture readiness via -progress pipe:1]].
- `stop()` → graceful `q` → SIGINT → SIGTERM → SIGKILL; returns `(ffmpeg_log, clean)`.
**Related:** [[Data Flow#Stopping FFmpeg gracefully]], [[Decisions (ADR)#ADR-8 Graceful FFmpeg shutdown]].

## browser.rs
**Role:** the browser session (owns Chromium).
**Key type:** `BrowserSession { browser, page, app_bundle, profile_dir, handler }`.
- `launch(chrome_path)` → headful Chromium, `--start-maximized`, **ephemeral `user_data_dir`**, `viewport(None)`; spawns the CDP `Handler` task; opens one reusable `about:blank` page; calls `maximize()`.
- `maximize()` → reads `window.screen.avail*`, sets explicit CDP window **bounds** (reliable on macOS). [[Decisions (ADR)#ADR-6 Reliable maximize via window bounds]].
- `bring_to_front()` → `Page.bringToFront` **and** `open -a <Chromium.app>`. [[Decisions (ADR)#ADR-10 Foreground via open -a]].
- `close()` → close browser, abort handler, delete the ephemeral profile.

## login.rs
**Role:** manual interactive login.
**Key:** `manual_login(session, login_url?)` → bring to front, optional `goto(login_url)`, prompt, **block on stdin Enter**. Session then lives in the reused page. [[Decisions (ADR)#ADR-5 Single reused page + manual login]].

## console.rs
**Role:** capture browser console + exceptions for failure diagnostics.
**Key type:** `ConsoleCapture` — subscribes to CDP `consoleAPICalled` + `exceptionThrown`, **serializes each event to JSON** (avoids brittle field access) into a buffer.
- `reset()` (per URL), `snapshot()` (for the error log). Used by [[Module Reference#orchestrator.rs|failure recovery]].

## viewer.rs
**Role:** make the page look "read" by a human.
**Key:** `view_page(page, cfg)` — loops: read metrics (`scrollY`, max, viewport height) → scroll ~one viewport in **small randomized steps** → **dwell** → maybe **pause** → maybe **scroll-up re-read**. At the bottom: wait for height to **settle** (lazy-load); if it grew, continue up to `max_infinite_extensions`. Bounded by `max_record`.
**Tuning:** all knobs live in [[Module Reference#config.rs]].
**Related:** [[Glossary#Lazy-load]], [[Glossary#Infinite scroll]].

## filename.rs
**Role:** safe, title-based filenames.
**Key:** `sanitize_slug`, `slug_from_url`, `build_filename(ts, title, url)` (final, ≤120 chars), `temp_filename(ts, url)` (while recording). Has unit tests. [[Decisions (ADR)#ADR-7 Temp filename then rename]].

## orchestrator.rs
**Role:** the per-URL queue loop — the conductor.
**Key:**
- `run_queue(...)` → iterate URLs, call `record_one`, log progress, `reporter.record` after each.
- `record_one(...)` → the exact [[Data Flow#Phase 2 — The per-URL loop (the heart of it)|per-URL ordering]]; returns a `UrlResult` (never throws per-URL).
- `failure(...)` → screenshot → `errors/<slug>.png`, console+ffmpeg log → `errors/<slug>.log`, builds a `failed` result. [[Decisions (ADR)#ADR-13 Sequential queue + per-URL failure recovery]].

## report.rs
**Role:** the run report.
**Key:** `Status` (success/failed), `UrlResult` (url, title, output_file, status, timestamps, duration, error?, screenshot?, console_log?, `forced_stop`), `Reporter` (writes `report.json` with a summary after every URL — crash-safe).

## error.rs
**Role:** typed **setup** errors (`thiserror`).
**Key:** `SetupError` — `FfmpegMissing`, `BrewMissing`, `Chromium`, `NoScreenDevice`, `BrowserLaunch`, `Other`. These **abort** the run (vs per-URL failures, which are recovered).

#recorder
