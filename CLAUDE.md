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
  - Apps may play to a different output than the Windows default (audio routing software), so loopback of the default can capture only Orbit's own beeps. Hence the game-sound device picker.
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
- [x] Step 20: app icon (app-icon.svg → npx tauri icon; static/logo.png + favicon), Home page (Home.svelte: replay/recording cards with timer via recording_since, hotkeys, storage_summary, latest captures), Settings split into categories (left nav), Gallery grouped by day + counts on chips, StatusBar.svelte (version, CPU/GPU/RAM via stats.rs: GetSystemTimes, PDH "\GPU Engine(*)\Utilization Percentage" busiest engine, GlobalMemoryStatusEx; every 2 s only while a window is open & not minimized; setting status_bar)
- [x] Step 21: in-game overlay (overlay.rs: transparent undecorated always-on-top window label "overlay" over the cursor's monitor, created on hotkey (default Alt+F8, only registered when overlay_enabled) and destroyed on close / Esc / click outside / focus loss; route src/routes/overlay; app.html makes it transparent before first paint; capabilities include "overlay"; overlay_screenshot closes overlay then waits 250 ms). Status bar shows ORBIT own memory (sum of private working set of orbit.exe + descendants) + PC CPU/GPU/RAM.
- [x] Step 22: region screenshot / snip (snip.rs: hotkey hotkey_snip default Alt+F11 → ffmpeg full frame of main monitor to $APPCACHE/snip/frozen.png, then borderless topmost window "snip" (route src/routes/snip) shows it; drag/move/resize selection; snip_save crops via FFmpeg, copies PNG to clipboard (tauri-plugin-clipboard-manager + tauri image-png feature), saves as normal screenshot with game name; Snip tile in overlay)
  - Step 22 follow-ups: snip + overlay windows are created with visible(false) and shown by the page (snip_ready after img.decode(), overlay_ready after tick()) with a 2.5 s safety net; snip window is transparent so an early frame shows the real screen, never black. BMP vs PNG for the frozen frame made no difference (~0.5 s is FFmpeg start + ddagrab). Snip: Copy (clipboard only, "snip-copied" toast), right-click menu, Ctrl+C. Overlay: no backdrop blur (pointless in a see-through window), quick settings panel, "All settings" → main window on Settings via PendingTab/take_pending_tab + "open-tab" event.
  - Bug fixed: hidden overlay got focus at creation (Tauri default focused=true), lost it immediately (Windows won't let invisible windows keep focus) → focus-loss handler destroyed it before it was ever shown. Now .focused(false) and focus-loss only closes when visible + had focus while visible.
- [ ] Step 23: performance check + about/updates/uninstall
  - Release build measured with scripts/measure-orbit.ps1 (orbit.exe + all child processes, private working set + CPU): window open 103 MB / 1.06 % CPU with status bar, 91 MB / 0.04 % without; closed to tray 6.9 MB / 0.00 %. Status bar cost came from WebView repainting (width transitions every 2 s) → now scaleX transform, no transition, Rust samples every 3 s and only emits when a shown value changes. Home reads the media folder once (storage_summary removed).
  - Check for updates: frontend fetch of api.github.com/repos/0kosmos/orbit/releases/latest (only on click; 404 while repo private / no releases), button in status bar + Settings → About & updates. Uninstall: about.rs runs uninstall.exe next to orbit.exe (NSIS), removes autostart, optional delete of app config/cache (never media). GitHub repo: https://github.com/0kosmos/orbit (private for now). No releases yet: need bundled FFmpeg + installer first.
  - Status bar cost root cause (measured with experiment switches): measuring itself ~0.04 %, every emit to the WebView ~0.1 s CPU (repaint/IPC). Fix: update only while an Orbit window is visible+focused, every 5 s, only noticeable changes (CPU/GPU ±3 %, RAM ±0.1 GB, Orbit ±2 MB, new minute) + CSS contain/will-change. Result: focused 0.15 %, background 0.01 %, tray 0 %.
- [ ] Step 24: first release v0.1.0 (NVIDIA only): FFmpeg 9.0.2 gyan.dev essentials (sha256 verified) bundled as resource src-tauri/ffmpeg/ffmpeg.exe (git-ignored, 105 MB) + LICENSE.txt; common::init_ffmpeg picks bundled exe else PATH; friendly_error maps NVENC/ddagrab errors; bundle targets ["nsis"] currentUser; README rewritten (features, measured performance, requirements, SmartScreen note, build steps, FFmpeg GPL notice). Orbit itself has no license chosen yet.
- [ ] Step 25: v0.1.1 audio sync fix. Sound was 0.55–0.7 s late idle (~1 s in CS): audio time 0 = when FFmpeg opened the pipe, video time 0 = first ddagrab frame after NVENC setup. FIRST attempt (hold sound back until first video frame) BROKE video with game+mic: ddagrab shares the -filter_complex graph with asplit/amix, so starving the audio inputs stalled screen capture (3 video frames in 9 s). FINAL fix: sound flows as before; audio::AvSync records sound start (capture thread) + video start (Background reads FFmpeg -progress pipe:1 / stats_period 0.25: first frame=N>0 → now − N/fps − 0.13 s); on save the sound start is cut off losslessly: common::add_aligned_inputs reads the input twice (video from 0, audio with -ss offset, -map 0:v -map 1:a, -c copy) — used in recorder to_mp4 and replay concat (offset kept per buffer session in Replay.av). Verified with flash+beep test (game+mic): 451 frames, +0.024 s for recording and simulated replay clip.
