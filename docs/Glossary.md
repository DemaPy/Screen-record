---
tags: [recorder, glossary, reference]
aliases: [Terms, Glossary]
---

# 📖 Glossary

Back to [[Recorder - Home]]. The vocabulary you need before reading [[Architecture]] / [[Data Flow]].

> [!info] How to use this note
> Skim it once. Come back when a term in another note is fuzzy. Terms link to where they're used.

### AVFoundation
Apple's media framework. FFmpeg uses its `avfoundation` *input device* to **capture the screen** (and cameras/mics). On macOS the only way we grab pixels. Devices are listed by index — see [[Module Reference#devices.rs]].

### CDP — Chrome DevTools Protocol
The JSON-over-WebSocket protocol Chrome/Chromium exposes for automation (navigate, evaluate JS, screenshot, move the window). [[Glossary#chromiumoxide]] speaks CDP for us. The same protocol the DevTools panel uses.

### chromiumoxide
The Rust crate that drives Chromium over [[Glossary#CDP — Chrome DevTools Protocol]]. There is **no official Playwright for Rust**, so this is our browser-automation library. See [[Decisions (ADR)#ADR-2 chromiumoxide over Playwright]].

### Headful (vs headless)
**Headful** = the browser has a *visible window*. We must be headful because we **film the window** off the screen. Headless (no window) would show nothing to capture.

### HEVC / H.265
A video codec — smaller files than H.264 at similar quality. We encode with it to keep 20-minute 4K recordings small. See [[Decisions (ADR)#ADR-4 Full-screen 480p HEVC hardware]].

### VideoToolbox
Apple's **hardware** video encode/decode framework. FFmpeg's `hevc_videotoolbox` encoder offloads HEVC encoding to the GPU/media engine → low CPU, important for long captures.

### `hvc1` tag
A metadata tag on the MP4 telling players "this is HEVC." Without `-tag:v hvc1`, QuickTime/Safari may refuse to play the file even though it's valid.

### moov atom
The MP4 index (where each frame lives). It's written **when recording stops cleanly**. If FFmpeg is hard-killed (SIGKILL) before writing it, the file is **unplayable**. This is why we stop FFmpeg gracefully — see [[Data Flow#Stopping FFmpeg gracefully]].

### Viewport
The visible area of the web page inside the window. We let it be the **real maximized window size** (not an emulated small viewport), so the captured page is large/legible.

### Lazy-load
Content that loads only when you scroll near it (images, comments). The [[Module Reference#viewer.rs]] waits for the page height to *settle* after scrolling so this content appears on camera.

### Infinite scroll
Pages that keep adding content as you reach the bottom (social feeds). We cap how many times we'll let the page grow, plus a hard time limit, so a feed can't record forever.

### `-progress pipe:1`
An FFmpeg option that prints machine-readable progress (newline-delimited `key=value`) to stdout. We parse it to know capture actually started. See the bug in [[Gotchas and Edge Cases#Readiness detection the carriage-return trap]].

### TCC
Apple's "Transparency, Consent, and Control" — the permission system behind those "X wants to record your screen / control your computer" prompts. **Screen Recording** is a TCC permission; granting it requires reopening the app. See [[Gotchas and Edge Cases#Black frames — Screen Recording permission]].

### Ephemeral profile
A throwaway Chromium user-data directory created per run and deleted on close. Keeps runs isolated and avoids restoring old tabs. See [[Decisions (ADR)#ADR-9 Ephemeral per-run profile]].

#recorder
