<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="110" alt="Orbit logo" />
</p>

<h1 align="center">Orbit</h1>

<p align="center">
  <b>A super lightweight game recorder for Windows.</b><br />
  Screenshots, snips, clips and instant replays, with almost no impact on your FPS.
</p>

<p align="center">
  <a href="https://github.com/0kosmos/orbit/releases/latest"><b>⬇ Download the latest version</b></a>
</p>

---

## Why Orbit?

Most recording and clipping apps run with a lot of background baggage. Orbit is built to
stay out of your way:

- **Light on memory.** About **50–200 MB** while running, depending on what you're doing,
  and **about 7 MB** when the window is closed to the tray.
- **0 % CPU when idle.** Nothing polls in the background. Everything reacts to your hotkeys.
- **Recording runs on your GPU.** Video is encoded by NVIDIA's encoder (NVENC), so your CPU
  stays free for the game.
- **Anti-cheat safe.** Orbit never injects into or hooks games. It only uses standard
  Windows screen capture, the same way Windows' own tools do.
- **Clean, minimal interface.** Black, dark grey or light theme, with every animation and
  effect switchable off.

## Features

| | |
|---|---|
| 📸 **Screenshots** | One hotkey, saved as PNG or JPG, named after the game you're playing. |
| ✂️ **Snip** | Freeze the screen, drag the area you want, then save and/or copy it to the clipboard. |
| ⏺️ **Recording** | Start and stop with a hotkey. 30–144 fps, quality presets, mouse cursor on/off. |
| ⏪ **Replay buffer** | Always keeps the last 15 s – 20 min. Press a hotkey to save what just happened. |
| 🔊 **Game sound + mic** | Pick any output device and mic. Mixed track plus separate tracks for editing. |
| 🎮 **Per-game gallery** | Everything sorted by game and date, with thumbnails, lengths, filters and bulk delete. |
| ▶️ **Built-in player** | Smooth playback, 0.5× / 1× / 2×, ±10 s, trimming, and crop / resize for screenshots. |
| 🕹️ **In-game overlay** | Optional (off by default). Quick buttons, quick settings and PC usage over your game. |
| 🧭 **Tray & autostart** | Close to the tray (frees the window's memory), start with Windows, check for updates. |

## Performance

Measured on the release build (i7-3770, GTX 1650 Ti, 16 GB RAM). The numbers cover Orbit
plus every process it starts (its window and FFmpeg).

| Situation | RAM | CPU |
|---|---|---|
| Closed to the tray (hotkeys and replay still ready) | ~7 MB | 0.00 % |
| Window open, in the background | ~80 MB | ~0.01 % |
| Window open and in use | ~80–120 MB | ~0.15 % |

In-game test while recording at 60 fps: 220 FPS without Orbit, 205–213 FPS while recording.

You can measure it yourself with [`scripts/measure-orbit.ps1`](scripts/measure-orbit.ps1).

## Requirements

- Windows 10 or 11, 64-bit
- **An NVIDIA graphics card with NVENC** (GTX 900 series or newer, all GTX 16xx and RTX cards).
  AMD and Intel support is planned.
- Microsoft Edge WebView2 (already part of Windows 10/11; the installer adds it if missing)

## Install

1. Download `Orbit_x.y.z_x64-setup.exe` from the
   [latest release](https://github.com/0kosmos/orbit/releases/latest).
2. Run it. No admin rights needed.
3. Windows may show **"Windows protected your PC"**, because Orbit isn't code-signed yet.
   Click **More info → Run anyway**.

Your files go to `Videos\Orbit` (changeable in Settings). Uninstall from Windows Settings
→ Apps, or from Orbit's Settings → About. Your screenshots and clips are always kept.

## Default hotkeys

| Action | Hotkey |
|---|---|
| Screenshot | `Alt + F12` |
| Snip (region screenshot) | `Alt + F11` |
| Start / stop recording | `Alt + F9` |
| Save replay | `Alt + F10` |
| In-game overlay (when enabled) | `Alt + F8` |

All hotkeys can be changed in Settings → Hotkeys.

## Privacy

Orbit has no account, no telemetry and no ads. It only goes online when **you** click
"Check for updates", to ask GitHub for the newest release.

## Building from source

Requirements: [Node.js](https://nodejs.org), [Rust](https://rustup.rs) and the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

1. Get FFmpeg (it's not in the repository because of its size): download the
   **essentials** build from [gyan.dev](https://www.gyan.dev/ffmpeg/builds/) and put
   `ffmpeg.exe` into `src-tauri/ffmpeg/`. During development, an FFmpeg on your PATH works too.
2. Install and run:
   ```
   npm install
   npm run tauri dev
   ```
3. Build the installer:
   ```
   npm run tauri build
   ```
   The setup file ends up in `src-tauri/target/release/bundle/nsis/`.

Built with [Tauri 2](https://tauri.app), [Svelte 5](https://svelte.dev) and Rust.

## Roadmap

- AMD (AMF) and Intel (Quick Sync) encoders
- Frame-exact clip trimming
- Automatic cleanup (delete clips older than X days / keep the folder under X GB)

## Third-party software

Orbit ships with [FFmpeg](https://ffmpeg.org) (the
[gyan.dev](https://www.gyan.dev/ffmpeg/builds/) "essentials" build), licensed under the
**GNU GPL v3**. Orbit runs it as a separate program. Its license is included with the app
(`ffmpeg/LICENSE.txt`), and its source code is available from
[ffmpeg.org](https://ffmpeg.org/download.html) and gyan.dev.
