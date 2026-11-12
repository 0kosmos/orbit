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
- [x] Step 4: replay buffer (sidebar toggle, Alt+F10 saves MP4 → Videos\Orbit\Clips; 5s .ts segments with -segment_wrap in app cache dir; settings.json for replay length + autostart)
- [x] Step 5: simple gallery (lists Videos\Orbit\{Screenshots,Clips,Recordings}; lazy 320px JPG thumbs via FFmpeg cached in app cache\thumbs, max 3 at once; click opens in default Windows app; asset protocol scoped to $APPCACHE/thumbs)
- [x] Step 6: save sound feedback (built-in WAVs played via Windows PlaySound; on/off setting)
- [x] Step 7: game/PC audio (WASAPI loopback in Rust → named pipe → FFmpeg AAC; silence padded by clock). FFmpeg build has no WASAPI input, only dshow (mic visible there).
- [x] Step 8: microphone (WASAPI capture → second pipe; off by default, device picker in Settings) + output-device picker for game sound + "Test" level meter for both
  - Track layout with game + mic: 1 "Game + Mic" (amix normalize=0; most players only play track 1), 2 "Game", 3 "Mic". User chose this over mic-only-separate.
  - User's PC uses Voicemeeter: Windows default output = "Voicemeeter AUX Input", but their apps play elsewhere → loopback of the default captured only Orbit's own beeps (files peaked at exactly -30 dB). Hence the game-sound device picker.
- [x] Step 9: per-game filter (foreground window → exe ProductName/FileDescription via game.rs; file names "<Game>_<YYYY-MM-DD_HH-MM-SS-mmm>.ext"; old "Orbit_..." files = "Other"; gallery game dropdown)
- [x] Step 10: gallery extras: video length badge (parsed from FFmpeg "Duration:" during thumbnail run, cached as thumbs\<file>.len), hover delete → Recycle Bin (SHFileOperationW + FOF_ALLOWUNDO), sort newest/oldest/largest/smallest
- [ ] Step 11: changeable hotkeys (settings hotkey_screenshot/record/replay as KeyboardEvent.code strings e.g. "Ctrl+Shift+KeyS"; hotkeys.rs applies/validates, pauses while capturing in Settings; sidebar hints follow "settings-changed" event)
