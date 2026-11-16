//! How far a running download has got, live, as the Bundle works it out (its `SteamDownloads.DownloadedSoFar`):
//! Steam leaves its manifest's byte counts as they were for most of a download (`crate::installs`), but writes what
//! it downloads to disk as it goes, one app at a time, staged whole. So what the client's own process
//! (`steam.exe`, the Steam at the source's folder and no other) has written since the download was first seen
//! running is what it has staged since then. That, on top of what the manifest had staged then, is turned into
//! what is downloaded by the two totals' share, Steam downloading compressed what it stages whole: on the real
//! machine Portal 2 was `download 0/7545611264, stage 0/12759257499`.
//!
//! It is a reasoned count, the Bundle's, not one read from Steam: whatever else the client writes meanwhile counts
//! too. `--log-queue-sources` prints it beside every other candidate once a second (`crate::probe`), so one install
//! on a real Steam says how near it is.
//!
//! - With no such process to ask (a fake Steam, whose client is a stand-in that has exited), the manifest's own
//!   counts stand, as the Bundle's do.
//! - With one, until its writes first move and while the manifest says nothing is down, nothing is known: the
//!   download reads as having no whole (`Downloading { 0, 0 }`), which the screens show as "Installing..." with no
//!   share and an empty bar, rather than a share that stands still at 0.0% (#132).
use model::InstallState;

use crate::installs::Counts;

/// What the client's process has written, in bytes, since it started: its I/O counters' `WriteTransferCount`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Writes {
    pub pid: u32,
    pub bytes: u64,
}

/// Where a download's count starts: the client's writes when it was first seen running, and what the manifest had
/// staged then.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Baseline {
    pid: u32,
    writes: u64,
    staged: u64,
}

/// A running download's state, from the live count where there is one (the module's rules). `baseline` is the
/// download's own, kept between looks; a new client process, or writes that went back, start it again.
pub fn downloading(baseline: &mut Option<Baseline>, counts: &Counts, writes: Option<Writes>) -> InstallState {
    let manifest = InstallState::Downloading { done: counts.downloaded, total: counts.to_download };
    let Some(writes) = writes else {
        *baseline = None;
        return manifest;
    };
    let base = match *baseline {
        Some(base) if base.pid == writes.pid && writes.bytes >= base.writes => base,
        _ => *baseline.insert(Baseline { pid: writes.pid, writes: writes.bytes, staged: counts.staged }),
    };
    let written = writes.bytes - base.writes;
    if written == 0 && counts.downloaded == 0 {
        return InstallState::Downloading { done: 0, total: 0 };
    }
    let staged = base.staged.saturating_add(written);
    let live = match counts.to_stage {
        0 => staged,
        to_stage => (staged.min(to_stage) as u128 * counts.to_download as u128 / to_stage as u128) as u64,
    };
    InstallState::Downloading { done: counts.downloaded.max(live.min(counts.to_download)), total: counts.to_download }
}

/// The writes of the Steam client at `root` (`steam.exe` under it), if it is running and Windows will say.
#[cfg(windows)]
pub fn writes(root: &std::path::Path) -> Option<Writes> {
    let pid = win::steam_process(root)?;
    let counters = win::io_counters(pid)?;
    Some(Writes { pid, bytes: counters.WriteTransferCount })
}

#[cfg(not(windows))]
pub fn writes(_root: &std::path::Path) -> Option<Writes> {
    None
}

#[cfg(windows)]
pub(crate) mod win {
    use std::path::{Path, PathBuf};

    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        GetProcessIoCounters, OpenProcess, QueryFullProcessImageNameW, IO_COUNTERS, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    /// The path of a process's program, if it can be asked.
    pub fn program(pid: u32) -> Option<PathBuf> {
        // SAFETY: the handle is checked and closed; the buffer's length is passed with it.
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process.is_null() {
                return None;
            }
            let mut path = [0u16; 1024];
            let mut len = path.len() as u32;
            let ok = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut len);
            CloseHandle(process);
            (ok != 0).then(|| PathBuf::from(String::from_utf16_lossy(&path[..len as usize])))
        }
    }

    /// The process of the Steam client at `root`: a `steam.exe` whose program is under it.
    pub fn steam_process(root: &Path) -> Option<u32> {
        // SAFETY: the snapshot is checked and closed; the entry is sized as the call wants.
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot.is_null() || snapshot as isize == -1 {
                return None;
            }
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut found = None;
            let mut more = Process32FirstW(snapshot, &mut entry) != 0;
            while more && found.is_none() {
                let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                if name.eq_ignore_ascii_case("steam.exe")
                    && program(entry.th32ProcessID).is_some_and(|path| crate::under(root, &path))
                {
                    found = Some(entry.th32ProcessID);
                }
                more = Process32NextW(snapshot, &mut entry) != 0;
            }
            CloseHandle(snapshot);
            found
        }
    }

    /// A process's I/O counters, if Windows will say.
    pub fn io_counters(pid: u32) -> Option<IO_COUNTERS> {
        // SAFETY: the handle is checked and closed; the counters are written by the call.
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process.is_null() {
                return None;
            }
            let mut counters: IO_COUNTERS = std::mem::zeroed();
            let ok = GetProcessIoCounters(process, &mut counters);
            CloseHandle(process);
            (ok != 0).then_some(counters)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Portal 2 on the real machine: 7.5 GB to download, 12.8 GB to stage.
    fn portal(downloaded: u64, staged: u64) -> Counts {
        Counts { flags: 1030, downloaded, to_download: 7_545_611_264, staged, to_stage: 12_759_257_499 }
    }

    fn down(state: InstallState) -> (u64, u64) {
        state.bytes().unwrap()
    }

    #[test]
    fn with_no_client_to_ask_the_manifest_stands() {
        let mut baseline = None;
        assert_eq!(down(downloading(&mut baseline, &portal(0, 0), None)), (0, 7_545_611_264));
        assert_eq!(baseline, None);
    }

    #[test]
    fn the_clients_writes_since_the_download_was_first_seen_count_as_staged_and_are_turned_into_downloaded() {
        let mut baseline = None;
        let at = |bytes| Some(Writes { pid: 7, bytes });
        // First seen: nothing written since, and nothing down by the manifest: no whole yet, not a standing 0.0%.
        assert_eq!(
            downloading(&mut baseline, &portal(0, 0), at(5_000)),
            InstallState::Downloading { done: 0, total: 0 }
        );
        // 1.27 GB staged since: a tenth of what is to stage, so a tenth of what is to download.
        let tenth = 1_275_925_750;
        let (done, total) = down(downloading(&mut baseline, &portal(0, 0), at(5_000 + tenth)));
        assert_eq!(total, 7_545_611_264);
        assert!(done.abs_diff(754_561_126) < 10, "{done}");
        // The manifest catching up, at 11%, ahead of the count: the more of the two.
        let (done, _) = down(downloading(&mut baseline, &portal(830_000_000, 1_400_000_000), at(5_000 + tenth)));
        assert_eq!(done, 830_000_000);
        // Never past the whole, however much more is written.
        let (done, total) = down(downloading(&mut baseline, &portal(0, 0), at(u64::MAX / 2)));
        assert_eq!(done, total);
    }

    #[test]
    fn a_new_client_or_writes_that_went_back_start_the_count_again_from_what_the_manifest_has_staged() {
        let mut baseline = None;
        downloading(&mut baseline, &portal(0, 0), Some(Writes { pid: 7, bytes: 1_000_000 }));
        // Steam restarted: its counters start again, from what the manifest had staged when it stopped.
        let staged = 6_379_628_749;
        let state = downloading(&mut baseline, &portal(3_772_805_632, staged), Some(Writes { pid: 9, bytes: 100 }));
        assert_eq!(down(state).0, 3_772_805_632);
        let state = downloading(
            &mut baseline,
            &portal(3_772_805_632, staged),
            Some(Writes { pid: 9, bytes: 100 + (12_759_257_499 - staged) }),
        );
        assert_eq!(down(state).0, 7_545_611_264);
    }

    #[test]
    fn with_nothing_to_stage_the_writes_are_what_is_downloaded() {
        let counts = Counts { flags: 1030, downloaded: 0, to_download: 1000, staged: 0, to_stage: 0 };
        let mut baseline = None;
        downloading(&mut baseline, &counts, Some(Writes { pid: 1, bytes: 0 }));
        assert_eq!(down(downloading(&mut baseline, &counts, Some(Writes { pid: 1, bytes: 250 }))), (250, 1000));
    }
}
