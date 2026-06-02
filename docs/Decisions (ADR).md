---
tags: [recorder, decisions, adr]
aliases: [Decisions, ADR, Architecture Decisions]
---

# 🧠 Decisions (ADR)

Back to [[Recorder - Home]]. Lightweight Architecture Decision Records — the **non-obvious calls** and *why*. Each: **Context → Decision → Consequence**. Many were discovered/validated by actually running the tool (see [[Gotchas and Edge Cases]]).

---

## ADR-1 Rust + tokio
- **Context:** User chose Rust. Long-running, manages external processes (ffmpeg) and async I/O (CDP).
- **Decision:** Rust with `tokio` (`full`).
- **Consequence:** Strong process/lifecycle control; no GC pauses on 20-min captures. Cost: no native Playwright (→ [[Decisions (ADR)#ADR-2 chromiumoxide over Playwright]]).

## ADR-2 chromiumoxide over Playwright
- **Context:** The spec said "use Playwright," but **Playwright has no official Rust binding**.
- **Decision:** Use **`chromiumoxide`** (pure-Rust [[Glossary#CDP — Chrome DevTools Protocol|CDP]] client). Rejected: the unofficial `playwright` crate (drags in a Node driver) and `fantoccini`/WebDriver (less control over scroll/console).
- **Consequence:** Idiomatic async Rust, direct control of navigation/scroll/screenshot/window bounds. Cost: we hit CDP/Chromium version quirks (the harmless `WS Invalid message` warnings).

## ADR-3 FFmpeg AVFoundation, not Playwright video
- **Context:** Need video of each page.
- **Decision:** Capture the **whole display** with FFmpeg's `avfoundation` device.
- **Consequence:** The browser and recorder are **decoupled** — the browser only needs to be visible + frontmost. This is the root reason [[Decisions (ADR)#ADR-6 Reliable maximize via window bounds|maximize]] and [[Decisions (ADR)#ADR-10 Foreground via open -a|foreground]] matter, and why [[Gotchas and Edge Cases#Black frames — Screen Recording permission|Screen Recording permission]] is mandatory.

## ADR-4 Full-screen 480p HEVC hardware
- **Context:** User has a **4K screen**; full-res 20-min videos are huge.
- **Decision:** Capture full screen, **downscale to 480p** (`scale=-2:480`), encode with **`hevc_videotoolbox`** (Mac hardware HEVC), tag `hvc1`, ~1.2 Mbit/s.
- **Consequence:** ~1 MB per ~6s observed → a 20-min video ≈ 150–200 MB. Hardware encode keeps CPU low (vital for long captures). Aspect ratio preserved (no 854×480 distortion). Plays in QuickTime/Safari thanks to `hvc1`.

## ADR-5 Single reused page + manual login
- **Context:** Must log in first, then record many pages **in the same session**.
- **Decision:** **One** `Browser` + **one** reused `Page` for the whole run. **Manual** login: open optional `--login-url`, user logs in by hand, presses Enter, queue runs.
- **Consequence:** Session/cookies persist across the queue automatically. Robust to captcha/2FA/varied forms (no brittle selectors). We never store credentials. A new context per URL would log us out — explicitly avoided.

## ADR-6 Reliable maximize via window bounds
- **Context:** Because we film the screen ([[Decisions (ADR)#ADR-3 FFmpeg AVFoundation, not Playwright video|ADR-3]]), the window must **fill the frame** or 480p is unreadable. `--start-maximized` and CDP `WindowState::Maximized` are **unreliable on macOS**.
- **Decision:** Read the usable screen rect from the renderer (`window.screen.avail*`) and set **explicit** CDP window bounds.
- **Consequence:** Window reliably fills the display. Verified by frame inspection during testing.

## ADR-7 Temp filename then rename
- **Context:** Final name needs the **page title**, but the title is known only **after** navigation, which is **after** recording starts.
- **Decision:** Record to a temp name (`<stamp>.part_<url-slug>.mp4`), then **rename** to `<stamp>_<title>.mp4` on stop. Names sanitized to `[a-z0-9-]`, capped at 120 chars.
- **Consequence:** Filenames are clean and title-based without reordering the capture/navigation steps. See [[Module Reference#filename.rs]].

## ADR-8 Graceful FFmpeg shutdown
- **Context:** A hard kill truncates the [[Glossary#moov atom]] → unplayable MP4. Across 100 URLs, leaked ffmpeg processes pile up.
- **Decision:** Stop sequence `q` → **SIGINT** → SIGTERM → **SIGKILL (last resort)**, each with a timeout. Every start has a matching stop+wait.
- **Consequence:** Files finalize and play; no orphan processes (verified). `forced_stop: true` flags the rare SIGKILL case.

## ADR-9 Ephemeral per-run profile
- **Context:** chromiumoxide reuses a **fixed-name** temp profile, so each run **session-restored the previous run's tabs** (observed: a stray Wikipedia tab).
- **Decision:** Give each run a **unique** `user_data_dir` (`recorder-profile-<pid>`), deleted on close.
- **Consequence:** Clean single window per run, isolated concurrent runs, user's real Chrome profile untouched. In-run login still persists (same process).

## ADR-10 Foreground via `open -a`
- **Context:** `Page.bringToFront` raises the **tab inside Chromium** but does **not** raise Chromium above other macOS apps. Whatever app is frontmost gets filmed (we accidentally filmed the editor once).
- **Decision:** Per URL, also run `open -a <Chromium.app>` to activate the app.
- **Consequence:** Chromium reliably frontmost. Chose `open -a` over `osascript … activate` because it needs **no Automation ([[Glossary#TCC]]) permission** and won't spawn a stray window. Note: this raises at the **start** of each URL, not continuously during one recording.

## ADR-11 Capture readiness via `-progress pipe:1`
- **Context:** First attempt parsed `frame=` from FFmpeg **stderr**, but stderr stats use **carriage returns** (`\r`), so the line reader never saw them → every recording falsely "produced no frames."
- **Decision:** Use `-progress pipe:1` (newline-delimited) on **stdout**; ready when `frame >= 1`.
- **Consequence:** Reliable readiness signal; doubles as the permission-hang detector. Full story in [[Gotchas and Edge Cases#Readiness detection the carriage-return trap]].

## ADR-12 Auto-install prerequisites
- **Context:** Onboarding friction; tools may be missing.
- **Decision:** Auto-`brew install ffmpeg`; auto-download Chromium via chromiumoxide's fetcher (cached).
- **Consequence:** Near-zero setup. Cost: first run downloads Chromium (~slow once). Homebrew is still required for ffmpeg.

## ADR-14 De-automate Chrome (avoid bot detection)
- **Context:** Sites (notably Google/YouTube) detected automation: the **"Chrome is being controlled by automated test software" infobar** appeared in-frame, and pages threw "prove you're a person" captchas. Two root causes, both from chromiumoxide's `DEFAULT_ARGS`: `--enable-automation` (infobar + `navigator.webdriver = true`) and `--enable-blink-features=IdleDetection`.
- **Decision — three measured, minimal layers** (validated empirically, not theorized):
  1. **Drop the automation flags.** `disable_default_args()` then re-add a **curated subset** without those two — `CURATED_DEFAULT_ARGS` in [[Module Reference#browser.rs]]. (CDP still works: it rides on `--remote-debugging-port`, added separately.) Kills the infobar.
  2. **Hide `navigator.webdriver`.** `--disable-blink-features=AutomationControlled` is **not** enough on Chrome 148 (verified: webdriver stayed `true`). So we inject `webdriver => undefined` via `addScriptToEvaluateOnNewDocument` — one injection covers all URLs ([[Decisions (ADR)#ADR-5 Single reused page + manual login|single reused page]]).
  3. **Prefer real Google Chrome** over fetched Chromium ([[Decisions (ADR)#ADR-12 Auto-install prerequisites|ensure_browser]]): genuine "Google Chrome" UA brand, codecs, Widevine — far less suspicious to Google. **Safety guard:** only when no personal Chrome is *already running* — real Chrome shares its `.app` bundle with the user's everyday browser, and [[Decisions (ADR)#ADR-10 Foreground via open -a|`open -a`]] would otherwise foreground (and film) their personal windows. If personal Chrome is up, we fall back to the uniquely-bundled fetched Chromium and warn; quit Chrome to get the anti-detection benefit.
- **Deliberately NOT done:** faking `navigator.plugins`/`languages`/`window.chrome`/WebGL. Those are *headless* stealth tricks; in **headful real Chrome** those values are already authentic, so faking them *adds* detectable inconsistencies. We patch only the one value (`webdriver`) that is genuinely a lie.
- **Consequence:** infobar gone; `navigator.webdriver` reads `undefined`; UA/brands authentic. **Honest limitation:** Google's detection is reputation-based too — a zero-history ephemeral profile ([[Decisions (ADR)#ADR-9 Ephemeral per-run profile]]) still looks fresh, so the **manual logged-in session** does the heavy lifting against captchas. No flag set *guarantees* zero captchas on Google. See [[Gotchas and Edge Cases#Automation detected — infobar / captcha]].

## ADR-13 Sequential queue + per-URL failure recovery
- **Context:** 100+ URLs; one bad page shouldn't sink the batch. Parallel screen-capture of one display is meaningless.
- **Decision:** Process **sequentially**; wrap each URL so failures capture a screenshot + logs and the loop continues. Write `report.json` after every URL.
- **Consequence:** Resilient long runs; crash-safe report. See [[Module Reference#orchestrator.rs]].

#recorder
