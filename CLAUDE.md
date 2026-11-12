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
- [x] Step 11: changeable hotkeys (settings hotkey_screenshot/record/replay as KeyboardEvent.code strings e.g. "Ctrl+Shift+KeyS"; hotkeys.rs applies/validates, pauses while capturing in Settings; sidebar hints follow "settings-changed" event)
- [x] Step 12: in-app viewer (Viewer.svelte overlay: video/img via asset protocol, scope now also $VIDEO/Orbit/**; ←/→/Esc; show in folder/open/delete). Recordings remuxed MKV→MP4 after stop (track names as title + handler_name); old MKVs may not play in-app → "Open in Windows" fallback.
- [x] Step 13: capture settings: fps 30-144, quality (cq 30/26/23/19), NVENC preset p1/p4/p6, draw_mouse, audio bitrate, screenshot PNG/JPG; replay length slider 15s-20min; any capture change auto-restarts replay buffer. GPU downscale NOT possible on user PC (scale_d3d11 "Could not create the texture", no CUDA hwmap) → no resolution option. ddagrab 120fps only reached ~75 on desktop (monitor 165 Hz).
- [x] Step 14: UI redesign: src/app.css design tokens per theme (black/grey/light via <html data-theme>), data-animations=off kills all transitions, data-blur=off makes .glass solid; ui.svelte.ts applyUi + ms() for Svelte transitions; settings theme/animations/blur; Windows title bar follows theme (set_theme). No infinite animations (idle CPU).
  - Step 14 follow-ups: accent is white (dark themes) / black (light); card hover = plain --hover-border, no movement; toast solid (no backdrop-filter while animating); custom Player.svelte (timeline, ±10s, play, 0.5/1/2x, volume, fullscreen, Space); record/replay toggles + list_media run off the main thread (user saw window freeze on stopping a recording until refocus).
- [x] Step 15: gallery bulk select (Select button / Ctrl+click / Shift+click range / Ctrl+A, bottom bar with 2-click confirm delete → delete_media_many to Recycle Bin; only visible selected files count) + new round outline trash button with lid animation
- [x] Step 16: screenshot crop/resize (ImageEditor.svelte in viewer: drag frame + corner handles, aspect Free/16:9/4:3/1:1, size 100/75/50/25%, PNG/JPG, "Save as copy" or "Replace original"; Rust media::edit_image via FFmpeg crop+scale lanczos, jpg yuvj444p q2; emits screenshot-saved; ?v= cache-busting on viewer src and thumbs)
- [x] Step 17: clip trimming (Trim button in viewer → Player trim mode with start/end handles, loops kept part; Rust media::trim_video = ffmpeg -ss before -i, -c copy -map 0, snaps to 1s keyframes; Save as copy / Replace original; player unmounted while saving so the file is not locked)
- [x] Step 18: system tray (tray.rs: icon + menu open/record/replay/save/quit; window has "create": false and is built from config by show_main_window; closing destroys the window = WebView2 freed; RunEvent::ExitRequested{code:None} → prevent_exit when close_to_tray); single-instance plugin; autostart plugin with --minimized (start_with_windows setting)
- [x] Step 19: save location (settings.save_dir, common::save_root/default_root; folder picker via tauri-plugin-dialog blocking_pick_folder in Rust; check_writable; asset_protocol_scope().allow_directory at startup + on change; existing files are NOT moved)
- [ ] Step 20: app icon (app-icon.svg → npx tauri icon; static/logo.png + favicon), Home page (Home.svelte: replay/recording cards with timer via recording_since, hotkeys, storage_summary, latest captures), Settings split into categories (left nav), Gallery grouped by day + counts on chips, StatusBar.svelte (version, CPU/GPU/RAM via stats.rs: GetSystemTimes, PDH "\GPU Engine(*)\Utilization Percentage" busiest engine, GlobalMemoryStatusEx; every 2 s only while a window is open & not minimized; setting status_bar)
- [ ] Step 21 (planned): in-game overlay, off by default, with compact StatusBar
