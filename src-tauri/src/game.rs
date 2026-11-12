//! Finds out which game (or program) you're playing, for file names and the gallery filter.
//!
//! Only asks Windows "which window is in front, and which .exe owns it", then reads
//! the program's name from that .exe file (like Explorer's file properties do).
//! Nothing touches the game itself, so this is anti-cheat safe.

use std::path::Path;
use windows_sys::Win32::Foundation::{CloseHandle, HWND};
use windows_sys::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
};

/// Programs whose .exe name says nothing about the game (e.g. Minecraft runs in Java),
/// so the window title is used instead.
const USE_WINDOW_TITLE: [&str; 3] = ["javaw", "java", "python"];

/// Name of the program in front, cleaned up for use in a file name.
/// "Desktop" for the desktop / Explorer / Orbit itself.
pub fn foreground_name() -> String {
    let name = detect().unwrap_or_default();
    let clean = clean(&name);
    if clean.is_empty() { "Desktop".into() } else { clean }
}

fn detect() -> Option<String> {
    // SAFETY: plain Windows calls; all buffers are sized and owned here.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 || pid == std::process::id() {
            return None;
        }

        let exe = process_path(pid);
        let stem = exe
            .as_deref()
            .and_then(|p| Path::new(p).file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let lower = stem.to_lowercase();

        if lower == "explorer" {
            return None;
        }
        if USE_WINDOW_TITLE.contains(&lower.as_str()) {
            return window_title(hwnd).or(Some(stem));
        }
        // Windows' own programs all have the product name "Microsoft Windows Operating
        // System", so for those the description ("Notepad") is used instead.
        exe.as_deref()
            .and_then(|p| {
                version_string(p, "ProductName")
                    .filter(|name| !name.contains("Operating System"))
                    .or_else(|| version_string(p, "FileDescription"))
            })
            .or(Some(stem).filter(|s| !s.is_empty()))
            .or_else(|| window_title(hwnd))
    }
}

/// Full path of a process' .exe.
unsafe fn process_path(pid: u32) -> Option<String> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return None;
    }
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(process, 0, buf.as_mut_ptr(), &mut len) };
    unsafe { CloseHandle(process) };
    (ok != 0).then(|| String::from_utf16_lossy(&buf[..len as usize]))
}

unsafe fn window_title(hwnd: HWND) -> Option<String> {
    let mut buf = [0u16; 256];
    let len = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    (len > 0).then(|| String::from_utf16_lossy(&buf[..len as usize]))
}

/// Reads a text field (like "ProductName") from an .exe's version information.
unsafe fn version_string(exe: &str, field: &str) -> Option<String> {
    let path: Vec<u16> = exe.encode_utf16().chain(Some(0)).collect();
    let size = unsafe { GetFileVersionInfoSizeW(path.as_ptr(), std::ptr::null_mut()) };
    if size == 0 {
        return None;
    }
    let mut data = vec![0u8; size as usize];
    if unsafe { GetFileVersionInfoW(path.as_ptr(), 0, size, data.as_mut_ptr().cast()) } == 0 {
        return None;
    }

    // Which language the texts are in (first entry of the translation table).
    let mut ptr = std::ptr::null_mut();
    let mut len = 0u32;
    let key: Vec<u16> = "\\VarFileInfo\\Translation".encode_utf16().chain(Some(0)).collect();
    if unsafe { VerQueryValueW(data.as_ptr().cast(), key.as_ptr(), &mut ptr, &mut len) } == 0
        || len < 4
    {
        return None;
    }
    let pair = ptr as *const u16;
    let (lang, codepage) = unsafe { (*pair, *pair.add(1)) };

    let key: Vec<u16> = format!("\\StringFileInfo\\{lang:04x}{codepage:04x}\\{field}")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    if unsafe { VerQueryValueW(data.as_ptr().cast(), key.as_ptr(), &mut ptr, &mut len) } == 0
        || len == 0
    {
        return None;
    }
    let text = unsafe { std::slice::from_raw_parts(ptr as *const u16, len as usize) };
    let text = String::from_utf16_lossy(text);
    let text = text.trim_end_matches('\0').trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// Keep only characters that are safe in Windows file names, max 40 characters.
fn clean(name: &str) -> String {
    let kept: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || " -.'&()+!".contains(*c))
        .collect();
    let words: Vec<&str> = kept.split_whitespace().collect();
    let joined = words.join(" ");
    let short: String = joined.chars().take(40).collect();
    short.trim_matches(|c: char| c == '.' || c.is_whitespace()).to_string()
}
