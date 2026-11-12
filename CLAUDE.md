# Orbit

Super lightweight game recording app for Windows. Main selling point: **almost zero performance cost** (unlike OBS/Medal).

Stack: Tauri 2 + SvelteKit (Svelte 5, SPA via adapter-static) + TypeScript. Created with `npm create tauri-app`.
The user also builds a sibling app, **Kosmos**, the same way.

## Features
- Screenshots (with resize option)
- Manual recording (start/stop)
- "Save last X minutes" replay buffer (user picks the length)
- Gallery tab of all screenshots and clips, with filtering per game

## Tech approach (tested and working on the user's PC)
- FFmpeg installed via winget (v9, gyan.dev full build), on PATH.
- Capture: `ddagrab` filter. Encode: `h264_nvenc` (GPU). Working command:
  `ffmpeg -y -filter_complex "ddagrab=framerate=60" -c:v h264_nvenc -cq 23 -t 40 "$env:USERPROFILE\Videos\test-game.mkv"`
- In-game test: 220 FPS idle → 205-213 FPS while recording, smooth clip, 2 dropped frames.
- User PC: i7-3770, 16GB DDR3, GTX 1650 Ti, Windows. Screen captured at 1720x1080.
- Rust is only a thin layer: start/stop FFmpeg, global hotkeys, file access.
- Replay buffer = FFmpeg writes short rolling segments; on save, join the last N minutes.

## Rules
- **Performance is the top priority**: low idle RAM, 0% idle CPU, no background polling, no heavy animations, lazy-load gallery and thumbnails.
- **Never inject into or hook games** (anti-cheat safety). Only standard Windows screen capture.
- Clean, minimal, black UI. Everything configurable in Settings later, but start minimal.
- Save feedback: short sound and/or small notification. An overlay (like NVIDIA Alt+Z) comes much later, optional, off by default.

## Build order (minimal version first)
1. Clean app shell (black theme, simple sidebar: Gallery, Settings)
2. Screenshot hotkey
3. Record / stop
4. Save last X minutes (replay buffer)
5. Simple gallery of screenshots and clips

Later: per-game filters, screenshot resize/crop, clip trimming, audio (game + mic), overlay, more settings.

## How to work with the user (beginner)
- Claude writes the code. Explain each step simply, say which files changed, and exactly how to test it.
- One small step at a time. **Wait for the user to confirm it works before moving on.**
- Say when to make a Git commit and give the exact commands; say whether they run separately or together.
- If the user pastes an error, fix it together.
- After each phase: explain how to measure total RAM (all processes incl. WebView2) and idle CPU, warn if something increases them, and suggest a cheaper option if there is one.

## Progress
- [x] Step 1: app shell
- [x] Step 2: screenshot hotkey (Alt+F12, FFmpeg ddagrab one frame → Videos\Orbit\Screenshots, ~0.5s; works in fullscreen game; CPU blip 0.1–0.2%)
- [x] Step 3: record / stop (Alt+F9 or sidebar button, MKV → Videos\Orbit\Recordings, stop by sending "q" to FFmpeg stdin)
- [ ] Step 4: replay buffer
