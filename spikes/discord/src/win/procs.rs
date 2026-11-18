//! What the web view's processes take: each one's kind (WebView2's own list), working set, private bytes and
//! processor time, and the sum; our own process beside them.
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::core::Interface;
use windows::Win32::Foundation::{CloseHandle, FILETIME};
use windows::Win32::System::ProcessStatus::{
    GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
};
use windows::Win32::System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

use crate::say;

pub struct Usage {
    pub pid: u32,
    pub kind: &'static str,
    pub working_set: u64,
    pub private: u64,
    /// Kernel and user time, in 100 ns.
    pub cpu: u64,
}

fn kind_name(kind: COREWEBVIEW2_PROCESS_KIND) -> &'static str {
    match kind.0 {
        0 => "browser",
        1 => "renderer",
        2 => "utility",
        3 => "sandbox helper",
        4 => "gpu",
        5 => "ppapi plugin",
        6 => "ppapi broker",
        _ => "other",
    }
}

pub fn webview(env: &ICoreWebView2Environment) -> Vec<Usage> {
    let mut out = Vec::new();
    let Ok(env8) = env.cast::<ICoreWebView2Environment8>() else {
        say!("memory: this runtime cannot list its processes (no ICoreWebView2Environment8)");
        return out;
    };
    unsafe {
        let Ok(infos) = env8.GetProcessInfos() else { return out };
        let mut count = 0u32;
        let _ = infos.Count(&mut count);
        for i in 0..count {
            let Ok(info) = infos.GetValueAtIndex(i) else { continue };
            let mut pid = 0i32;
            let mut kind = COREWEBVIEW2_PROCESS_KIND::default();
            let _ = info.ProcessId(&mut pid);
            let _ = info.Kind(&mut kind);
            if let Some(usage) = usage(pid as u32, kind_name(kind)) {
                out.push(usage);
            }
        }
    }
    out
}

pub fn own() -> Option<Usage> {
    usage(std::process::id(), "this spike")
}

fn usage(pid: u32, kind: &'static str) -> Option<Usage> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut counters = PROCESS_MEMORY_COUNTERS_EX {
            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
            ..Default::default()
        };
        let memory = GetProcessMemoryInfo(
            handle,
            &mut counters as *mut PROCESS_MEMORY_COUNTERS_EX as *mut PROCESS_MEMORY_COUNTERS,
            counters.cb,
        );
        let (mut a, mut b, mut kernel, mut user) =
            (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        let times = GetProcessTimes(handle, &mut a, &mut b, &mut kernel, &mut user);
        let _ = CloseHandle(handle);
        memory.ok()?;
        let ft = |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
        Some(Usage {
            pid,
            kind,
            working_set: counters.WorkingSetSize as u64,
            private: counters.PrivateUsage as u64,
            cpu: if times.is_ok() { ft(kernel) + ft(user) } else { 0 },
        })
    }
}

fn mb(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_048_576.0)
}

/// Logs each process and the sum; gives the web view's processes' total processor time.
pub fn report(env: &ICoreWebView2Environment, when: &str) -> u64 {
    let list = webview(env);
    for u in &list {
        say!("memory ({when}): {} pid {}: working set {}, private {}", u.kind, u.pid, mb(u.working_set), mb(u.private));
    }
    let (ws, private, cpu) = list.iter().fold((0, 0, 0), |(w, p, c), u| (w + u.working_set, p + u.private, c + u.cpu));
    say!("memory ({when}): web view, {} processes: working set {}, private {}", list.len(), mb(ws), mb(private));
    if let Some(me) = own() {
        say!("memory ({when}): this spike: working set {}, private {}", mb(me.working_set), mb(me.private));
    }
    cpu
}

/// The web view's processor time summed, in 100 ns.
pub fn cpu(env: &ICoreWebView2Environment) -> u64 {
    webview(env).iter().map(|u| u.cpu).sum()
}
