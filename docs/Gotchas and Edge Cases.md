---
tags: [recorder, gotchas, debugging, edge-cases]
aliases: [Gotchas, Edge Cases, Pitfalls]
---

# ⚠️ Gotchas and Edge Cases

Back to [[Recorder - Home]]. Real traps found while building/testing this. Each cost debugging time — read before you "fix" something that's already handled.

---

## Black frames — Screen Recording permission
- **Symptom:** FFmpeg **hangs** (never produces frames); the tool reports every URL as `failed: "ffmpeg never produced frames…"`. Or you get a black video.
- **Cause:** macOS [[Glossary#TCC]] gates screen capture. The permission attaches to the **host app** of the process — if you run from **Cursor/VS Code's integrated terminal**, you must grant **Cursor/VS Code** (not Terminal.app).
- **Critical detail:** the grant only takes effect after you **fully quit and reopen** that app. A running process keeps the old (denied) state.
- **Fix:** System Settings → Privacy & Security → Screen Recording → enable the host app → reopen it.
- **How we handle it:** [[Module Reference#ffmpeg.rs|wait_until_ready]] returns `false` when no frames flow → that URL **fails fast** with an actionable message instead of silently writing a black file ([[Decisions (ADR)#ADR-11 Capture readiness via -progress pipe:1]]).

## Readiness detection: the carriage-return trap
- **Symptom (during dev):** every recording falsely reported "no frames," even though FFmpeg was clearly capturing.
- **Cause:** FFmpeg's **stderr** progress stats (`frame= … fps= …`) are printed with **carriage returns (`\r`)**, not newlines. A line-by-line reader (`\n`) never sees them → readiness times out.
- **Fix:** use `-progress pipe:1` → newline-delimited `key=value` on **stdout**; ready when `frame >= 1`. See [[Decisions (ADR)#ADR-11 Capture readiness via -progress pipe:1]].
- **Lesson:** never scrape FFmpeg's human stats line; use `-progress`.

## Window must be maximized AND frontmost
Because we **film the display** ([[Decisions (ADR)#ADR-3 FFmpeg AVFoundation, not Playwright video]]), two independent things can go wrong:

1. **Not maximized** → the page is tiny in the 4K→480p downscale (unreadable).
   - `--start-maximized` and CDP `WindowState::Maximized` are **unreliable on macOS**.
   - **Fix:** set explicit window **bounds** from `window.screen.avail*` — [[Decisions (ADR)#ADR-6 Reliable maximize via window bounds]].
2. **Not frontmost** → we film whatever app *is* in front.
   - `Page.bringToFront` only raises the **tab**, not the macOS window above other apps.
   - **Fix:** `open -a <Chromium.app>` per URL — [[Decisions (ADR)#ADR-10 Foreground via open -a]].
   - **Residual limitation:** we re-foreground at the **start** of each URL, not continuously. If something steals focus **mid-recording**, that URL captures the wrong thing. → **Don't use the machine during a run.** (We literally filmed the code editor once, because a command was run in it mid-capture.)

## Stray tabs / session restored across runs
- **Symptom:** a previous run's tab (e.g. a Wikipedia article) appears in a later run's window.
- **Cause:** chromiumoxide reuses a **fixed-name** temp profile, and Chromium restores its last session.
- **Fix:** unique **ephemeral** `user_data_dir` per run, deleted on close — [[Decisions (ADR)#ADR-9 Ephemeral per-run profile]]. In normal single-run use you'd never see this anyway (one reused tab).

## MP4 won't play
- **Cause A:** missing `-tag:v hvc1` → HEVC stream players reject. We always add it. [[Glossary#hvc1 tag]].
- **Cause B:** FFmpeg was **hard-killed** before writing the [[Glossary#moov atom]]. Check `forced_stop: true` in the report — that recording may be truncated. We escalate gracefully to avoid this ([[Decisions (ADR)#ADR-8 Graceful FFmpeg shutdown]]).

## Shell globbing of URLs
- **Symptom:** `zsh: no matches found: https://…/Rust_(programming_language)` — the command **aborts before running**.
- **Cause:** parentheses/`?`/`*` in a URL are shell glob characters.
- **Fix:** **quote** URLs on the command line: `cargo run -- "https://…/Rust_(programming_language)"`. (Inside `--file` they're fine — no shell involved.)

## `WS Invalid message` warnings
- **Symptom:** `WARN chromiumoxide::handler: WS Invalid message: data did not match any variant…`
- **Cause:** minor CDP schema mismatch between chromiumoxide and the fetched Chromium build. **Harmless** — those messages are dropped; navigation/evaluate/screenshot all work.
- **Action:** ignore (could be silenced by pinning a matching Chromium revision later).

## AVFoundation pixel-format notice
- **Symptom:** `Selected pixel format (yuv420p) is not supported by the input device… Overriding to uyvy422`.
- **Cause:** the capture device's native formats differ; FFmpeg negotiates `uyvy422` for input and still encodes our `yuv420p` output. **Harmless.**

## The two waits look redundant but aren't
`pre_record_wait` (2s, before starting ffmpeg) ≠ capture-readiness wait (after starting ffmpeg, before navigating). Removing either reintroduces a real bug. See [[Data Flow#The two warm-up waits (don't confuse them)]].

## Long pages take real time
A long article (e.g. the Rust Wikipedia page) recorded ~2:40 because the human-like viewer dwells per section. That's **intended**. The `max_record` (20 min) hard cap + `-t` protect against runaway/infinite-scroll pages ([[Glossary#Infinite scroll]]).

#recorder
