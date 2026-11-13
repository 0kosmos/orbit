//! PC usage for the status bar: CPU %, GPU %, RAM used / total.
//!
//! Measured every 5 seconds, but ONLY while Orbit's window (or the overlay) is open, in
//! focus and not minimized, and the status bar is switched on. With the window closed (tray) or minimized,
//! nothing is measured. Uses Windows' own counters (the ones Task Manager uses).

use crate::settings;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::ProcessStatus::{
    K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX2,
};
use windows_sys::Win32::System::Performance::{
    PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA,
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
    PdhOpenQueryW,
};
use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows_sys::Win32::System::Threading::{
    GetSystemTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};

#[derive(Clone, Serialize)]
pub struct Stats {
    /// 0–100
    cpu: f32,
    /// 0–100, None if Windows has no GPU counters
    gpu: Option<f32>,
    ram_used: u64,
    ram_total: u64,
    /// Memory used by Orbit itself: orbit.exe + its window (WebView2) + FFmpeg.
    orbit_ram: u64,
}

static RUNNING: AtomicBool = AtomicBool::new(false);

/// Start measuring (if not already). Called when a window opens or the setting is turned on.
pub fn ensure_running(app: &AppHandle) {
    if !settings::get(app).status_bar || RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let mut cpu = CpuMeter::new();
        let mut gpu = GpuMeter::new();
        let mut first = true;
        // What the window shows right now (last values sent), and the minute.
        let mut last_sent: Option<(Stats, String)> = None;
        loop {
            std::thread::sleep(Duration::from_millis(if first { 700 } else { 5000 }));
            first = false;

            // Stop completely when no window is open or the status bar was switched off.
            let windows = app.webview_windows();
            if windows.is_empty() || !settings::get(&app).status_bar {
                break;
            }
            // Only while you're looking at Orbit: a window (or the overlay) must be visible,
            // not minimized AND in focus. Orbit open behind a game or on another monitor
            // costs nothing (each update makes WebView2 redraw, ~0.1 s CPU). The very first
            // sample is always shown, so the bar isn't empty.
            let looked_at = windows.values().any(|w| {
                w.is_visible().unwrap_or(false)
                    && !w.is_minimized().unwrap_or(true)
                    && w.is_focused().unwrap_or(false)
            });
            if !looked_at && last_sent.is_some() {
                continue;
            }

            let (ram_used, ram_total) = ram();
            let stats = Stats {
                cpu: cpu.sample(),
                gpu: gpu.as_mut().and_then(|g| g.sample()),
                ram_used,
                ram_total,
                orbit_ram: orbit_memory(),
            };
            // Every update makes the window redraw, which costs far more than measuring
            // (measured: ~0.4 % CPU vs ~0.04 %). So only send NOTICEABLE changes: CPU / GPU
            // by 3 % or more, RAM by 0.1 GB, Orbit by 2 MB, or a new minute for the clock.
            let minute = chrono::Local::now().format("%H:%M").to_string();
            let noticeable = match &last_sent {
                None => true,
                Some((old, old_minute)) => {
                    (stats.cpu - old.cpu).abs() >= 3.0
                        || match (stats.gpu, old.gpu) {
                            (Some(a), Some(b)) => (a - b).abs() >= 3.0,
                            (a, b) => a.is_some() != b.is_some(),
                        }
                        || stats.ram_used.abs_diff(old.ram_used) >= 100_000_000
                        || stats.orbit_ram.abs_diff(old.orbit_ram) >= 2_000_000
                        || minute != *old_minute
                }
            };
            if noticeable {
                let _ = app.emit("stats", stats.clone());
                last_sent = Some((stats, minute));
            }
        }
        RUNNING.store(false, Ordering::SeqCst);
    });
}

/// Orbit's own memory, measured like Task Manager's "Memory" column (private working
/// set), added up over orbit.exe and every process it started (WebView2, FFmpeg).
fn orbit_memory() -> u64 {
    // All running processes with their parent.
    let mut procs: Vec<(u32, u32)> = Vec::new();
    // SAFETY: standard ToolHelp snapshot walk; the handle is closed at the end.
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return 0;
        }
        let mut e: PROCESSENTRY32W = std::mem::zeroed();
        e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut e) != 0;
        while ok {
            procs.push((e.th32ProcessID, e.th32ParentProcessID));
            ok = Process32NextW(snap, &mut e) != 0;
        }
        CloseHandle(snap);
    }

    // Orbit and everything below it (children, grandchildren, ...).
    let mut ours = vec![std::process::id()];
    let mut i = 0;
    while i < ours.len() {
        let parent = ours[i];
        for &(pid, ppid) in &procs {
            if ppid == parent && pid != parent && !ours.contains(&pid) {
                ours.push(pid);
            }
        }
        i += 1;
    }

    ours.iter()
        .map(|&pid| unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return 0;
            }
            let mut c: PROCESS_MEMORY_COUNTERS_EX2 = std::mem::zeroed();
            c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX2>() as u32;
            let ok = K32GetProcessMemoryInfo(h, &mut c as *mut _ as *mut PROCESS_MEMORY_COUNTERS, c.cb) != 0;
            CloseHandle(h);
            if ok { c.PrivateWorkingSetSize as u64 } else { 0 }
        })
        .sum()
}

fn ram() -> (u64, u64) {
    let mut m = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..unsafe { std::mem::zeroed() }
    };
    // SAFETY: `m` is a correctly sized MEMORYSTATUSEX.
    if unsafe { GlobalMemoryStatusEx(&mut m) } == 0 {
        return (0, 0);
    }
    (m.ullTotalPhys - m.ullAvailPhys, m.ullTotalPhys)
}

/// CPU: how much of the time since the last sample the CPUs were NOT idle.
struct CpuMeter {
    last: Option<(u64, u64)>, // (idle, total)
}

impl CpuMeter {
    fn new() -> Self {
        let mut m = Self { last: None };
        m.sample();
        m
    }

    fn sample(&mut self) -> f32 {
        let (mut idle, mut kernel, mut user) = unsafe { std::mem::zeroed::<(FILETIME, FILETIME, FILETIME)>() };
        // SAFETY: valid out-pointers.
        if unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } == 0 {
            return 0.0;
        }
        let ft = |f: FILETIME| ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64;
        // "kernel" time already includes idle time.
        let now = (ft(idle), ft(kernel) + ft(user));
        let usage = match self.last {
            Some((idle0, total0)) if now.1 > total0 => {
                let busy = (now.1 - total0).saturating_sub(now.0 - idle0);
                busy as f32 / (now.1 - total0) as f32 * 100.0
            }
            _ => 0.0,
        };
        self.last = Some(now);
        usage.clamp(0.0, 100.0)
    }
}

/// GPU: like Task Manager, the busiest GPU engine (3D, video encode, ...), with the
/// usage of all programs on that engine added up.
struct GpuMeter {
    query: PDH_HQUERY,
    counter: PDH_HCOUNTER,
}

// The PDH handles are only used from the one measuring thread.
unsafe impl Send for GpuMeter {}

impl GpuMeter {
    fn new() -> Option<Self> {
        let mut query: PDH_HQUERY = std::ptr::null_mut();
        let mut counter: PDH_HCOUNTER = std::ptr::null_mut();
        let path: Vec<u16> = r"\GPU Engine(*)\Utilization Percentage"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        // SAFETY: valid pointers; handles are closed in Drop.
        unsafe {
            if PdhOpenQueryW(std::ptr::null(), 0, &mut query) != 0 {
                return None;
            }
            if PdhAddEnglishCounterW(query, path.as_ptr(), 0, &mut counter) != 0 {
                PdhCloseQuery(query);
                return None;
            }
            // Usage is measured between two samples, so take a first one now.
            PdhCollectQueryData(query);
        }
        Some(Self { query, counter })
    }

    fn sample(&mut self) -> Option<f32> {
        // SAFETY: handles are valid; the buffer is sized as PDH asks.
        unsafe {
            if PdhCollectQueryData(self.query) != 0 {
                return None;
            }
            let mut size = 0u32;
            let mut count = 0u32;
            let status = PdhGetFormattedCounterArrayW(
                self.counter,
                PDH_FMT_DOUBLE,
                &mut size,
                &mut count,
                std::ptr::null_mut(),
            );
            if status != PDH_MORE_DATA || size == 0 {
                return None;
            }
            // u64 buffer = correctly aligned for the item structs.
            let mut buf = vec![0u64; (size as usize).div_ceil(8)];
            let items = buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
            if PdhGetFormattedCounterArrayW(self.counter, PDH_FMT_DOUBLE, &mut size, &mut count, items) != 0 {
                return None;
            }

            // Instance names look like "pid_123_luid_0x0_0xD1F5_phys_0_eng_3_engtype_3D".
            // Add up all programs per engine ("luid…eng_3"), then take the busiest engine.
            let mut per_engine: HashMap<String, f64> = HashMap::new();
            for item in std::slice::from_raw_parts(items, count as usize) {
                if item.FmtValue.CStatus != 0 {
                    continue;
                }
                let name = wide_to_string(item.szName);
                let engine = name
                    .find("luid_")
                    .map(|i| &name[i..])
                    .and_then(|s| s.split("_engtype").next())
                    .unwrap_or(&name)
                    .to_string();
                *per_engine.entry(engine).or_default() += item.FmtValue.Anonymous.doubleValue;
            }
            let busiest = per_engine.values().cloned().fold(0.0, f64::max);
            Some(busiest.clamp(0.0, 100.0) as f32)
        }
    }
}

impl Drop for GpuMeter {
    fn drop(&mut self) {
        unsafe { PdhCloseQuery(self.query) };
    }
}

unsafe fn wide_to_string(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut len = 0;
    while unsafe { *p.add(len) } != 0 {
        len += 1;
    }
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, len) })
}
