//! `--log-queue-sources`: every candidate for how far a Steam download has got, read once a second while one is
//! under way and printed side by side, so one install on a real Steam shows which of them count up (#132). The
//! manifest's counts were seen to stand still for most of a 7.5 GB download; the rest are what might not:
//!
//! - `manifest`: `StateFlags`, `BytesDownloaded` of `BytesToDownload`, `BytesStaged` of `BytesToStage`, as Steam
//!   last wrote them; `registry`, `Apps\<appid>\Updating`.
//! - `steam.exe`: the client's own I/O counters, bytes written, read and other, since it started; `live`, what
//!   `crate::live` makes of the writes (the Bundle's count, and the one the Queue shows).
//! - `downloading`: the files under `steamapps\downloading\<appid>` in every library (and `temp\<appid>`, should
//!   Steam stage there): how many, their length, what they take on disk (`GetCompressedFileSizeW`, which is less
//!   than the length for a sparse file), and how much of them is valid data (`FSCTL_QUERY_FILE_REGIONS`, cached and
//!   not), which for a file Steam made full length up front is how much it has written into it.
//! - `network`: bytes received by every hardware network adapter that is up: the PC's, not Steam's alone.
//! - `cef`: where the user has Steam's CEF debugger on, the client's own download overview
//!   (`SteamClient.Downloads.RegisterForDownloadOverview`), as it gives it.
//!
//! Each number has its change per second since the last look after it: `writes 1048576000 (+52428800/s)`.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::installs::{self, Counts};
use crate::{cef, live, overview};

/// Looks, one after another, at the Steam at a folder.
pub struct Probe {
    root: PathBuf,
    /// The registry is Steam's own: the install the registry names, not a fake's.
    registry: bool,
    start: Instant,
    /// Each number's last reading, by its name, with when.
    last: HashMap<String, (u64, Instant)>,
    baselines: HashMap<u32, live::Baseline>,
}

impl Probe {
    pub fn new(root: PathBuf, registry: bool) -> Self {
        Self { root, registry, start: Instant::now(), last: HashMap::new(), baselines: HashMap::new() }
    }

    /// A line for each install under way: `queue-sources: 620 at 12.0 s: manifest flags 1030, ...`. None while
    /// nothing is.
    pub fn look(&mut self) -> Vec<String> {
        let libraries = crate::libraries(&self.root);
        let registry = self.registry;
        let found = installs::under_way(&libraries, &|appid| registry && crate::registry::updating(appid));
        self.baselines.retain(|appid, _| found.iter().any(|(id, ..)| id == appid));
        // Steam's overview, whole: what it says of an app whose manifest says it is done is wanted too (#132).
        let read = cef::enabled(&self.root).then(|| overview::read(&cef::ports(&self.root)));
        let cef = read.as_ref().map(|read| match read {
            Ok(overview) => overview.raw.clone(),
            Err(why) => why.clone(),
        });
        if found.is_empty() {
            let at = self.start.elapsed().as_secs_f64();
            return match read {
                Some(Ok(overview)) if overview.working_on(overview.appid) => {
                    vec![format!("queue-sources: no manifest under way at {at:.1} s; cef {}", overview.raw)]
                }
                _ => Vec::new(),
            };
        }
        let io = win::steam_io(&self.root);
        let network = win::network_in();
        found
            .into_iter()
            .map(|(appid, state, counts)| {
                let mut parts = vec![format!("{state:?}")];
                parts.push(self.manifest(appid, &counts, registry && crate::registry::updating(appid)));
                parts.push(match io {
                    Some((pid, written, read, other)) => {
                        let writes = Some(live::Writes { pid, bytes: written });
                        let mut baseline = self.baselines.get(&appid).copied();
                        let live = live::downloading(&mut baseline, &counts, writes);
                        if let Some(baseline) = baseline {
                            self.baselines.insert(appid, baseline);
                        }
                        let live = live.bytes().map_or("none".into(), |(done, total)| format!("{done} of {total}"));
                        format!(
                            "steam.exe {pid} writes {}, reads {}, other {}; live {live}",
                            self.number(appid, "writes", written),
                            self.number(appid, "reads", read),
                            self.number(appid, "other", other),
                        )
                    }
                    None => "steam.exe not running here, or not to be asked".into(),
                });
                for folder in ["downloading", "temp"] {
                    let dirs: Vec<PathBuf> =
                        libraries.iter().map(|lib| lib.join(folder).join(appid.to_string())).collect();
                    if let Some(files) = win::files(&dirs) {
                        parts.push(format!(
                            "{folder} {} files, length {}, on disk {}, valid cached {}, valid {}{}",
                            files.count,
                            self.number(appid, &format!("{folder} length"), files.length),
                            self.number(appid, &format!("{folder} on disk"), files.on_disk),
                            self.number(appid, &format!("{folder} cached"), files.valid_cached),
                            self.number(appid, &format!("{folder} valid"), files.valid),
                            files.failed.map_or(String::new(), |why| format!(" ({why})")),
                        ));
                    }
                }
                if let Some(network) = network {
                    parts.push(format!("network in {}", self.number(appid, "network", network)));
                }
                parts.push(format!("cef {}", cef.as_deref().unwrap_or("off")));
                let at = self.start.elapsed().as_secs_f64();
                format!("queue-sources: {appid} at {at:.1} s: {}", parts.join("; "))
            })
            .collect()
    }

    fn manifest(&mut self, appid: u32, counts: &Counts, updating: bool) -> String {
        format!(
            "manifest flags {}, downloaded {} of {}, staged {} of {}; registry updating {}",
            counts.flags,
            self.number(appid, "downloaded", counts.downloaded),
            counts.to_download,
            self.number(appid, "staged", counts.staged),
            counts.to_stage,
            updating as u8,
        )
    }

    /// A number, and how fast it has changed since the last look: `1048576 (+52428/s)`.
    fn number(&mut self, appid: u32, name: &str, value: u64) -> String {
        let now = Instant::now();
        let last = self.last.insert(format!("{appid} {name}"), (value, now));
        match last {
            Some((was, at)) if now > at => {
                let rate = (value as f64 - was as f64) / (now - at).as_secs_f64();
                format!("{value} ({rate:+.0}/s)")
            }
            _ => value.to_string(),
        }
    }
}

/// What the files under a folder come to.
#[derive(Debug, Default)]
pub struct Files {
    pub count: u64,
    pub length: u64,
    pub on_disk: u64,
    pub valid_cached: u64,
    pub valid: u64,
    /// Why some could not be asked, the first of them, with how many.
    pub failed: Option<String>,
}

/// Every file under `dir`, at any depth.
fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => walk(&entry.path(), out),
            Ok(kind) if kind.is_file() => out.push(entry.path()),
            _ => {}
        }
    }
}

#[cfg(not(windows))]
mod win {
    use std::path::{Path, PathBuf};

    pub fn steam_io(_root: &Path) -> Option<(u32, u64, u64, u64)> {
        None
    }

    pub fn network_in() -> Option<u64> {
        None
    }

    /// The files' count and length; nothing else is asked off Windows.
    pub fn files(dirs: &[PathBuf]) -> Option<super::Files> {
        let mut paths = Vec::new();
        dirs.iter().filter(|dir| dir.is_dir()).for_each(|dir| super::walk(dir, &mut paths));
        if !dirs.iter().any(|dir| dir.is_dir()) {
            return None;
        }
        let length = paths.iter().filter_map(|path| path.metadata().ok()).map(|meta| meta.len()).sum();
        Some(super::Files { count: paths.len() as u64, length, ..Default::default() })
    }
}

#[cfg(windows)]
mod win {
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};
    use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, GetCompressedFileSizeW, FILE_FLAG_BACKUP_SEMANTICS, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE,
        FILE_SHARE_READ, FILE_SHARE_WRITE, INVALID_FILE_SIZE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Ioctl::{
        FILE_REGION_INFO, FILE_REGION_INPUT, FILE_REGION_OUTPUT, FILE_REGION_USAGE_VALID_CACHED_DATA,
        FILE_REGION_USAGE_VALID_NONCACHED_DATA, FSCTL_QUERY_FILE_REGIONS,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl;

    use super::Files;
    use crate::live::win::{io_counters, steam_process};

    /// The Steam client's pid and its bytes written, read and other.
    pub fn steam_io(root: &Path) -> Option<(u32, u64, u64, u64)> {
        let pid = steam_process(root)?;
        let io = io_counters(pid)?;
        Some((pid, io.WriteTransferCount, io.ReadTransferCount, io.OtherTransferCount))
    }

    /// Bytes received by every hardware adapter that is up.
    pub fn network_in() -> Option<u64> {
        // SAFETY: the table is the call's to allocate and is freed with FreeMibTable; rows are read within its count.
        unsafe {
            let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
            if GetIfTable2(&mut table) != 0 || table.is_null() {
                return None;
            }
            let count = (*table).NumEntries as usize;
            let rows = std::slice::from_raw_parts((*table).Table.as_ptr(), count);
            let total = rows
                .iter()
                .filter(|row| row.InterfaceAndOperStatusFlags._bitfield & 1 != 0 && row.OperStatus == IfOperStatusUp)
                .map(|row| row.InOctets)
                .sum();
            FreeMibTable(table as *const _);
            Some(total)
        }
    }

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    pub fn files(dirs: &[PathBuf]) -> Option<Files> {
        if !dirs.iter().any(|dir| dir.is_dir()) {
            return None;
        }
        let mut paths = Vec::new();
        dirs.iter().filter(|dir| dir.is_dir()).for_each(|dir| super::walk(dir, &mut paths));
        let mut files = Files { count: paths.len() as u64, ..Files::default() };
        let mut failed = (0, String::new());
        for path in &paths {
            let length = path.metadata().map(|meta| meta.len()).unwrap_or(0);
            files.length += length;
            let name = wide(path);
            // SAFETY: the name is NUL-terminated; the high half is written by the call.
            unsafe {
                let mut high = 0u32;
                let low = GetCompressedFileSizeW(name.as_ptr(), &mut high);
                if low != INVALID_FILE_SIZE || GetLastError() == 0 {
                    files.on_disk += (high as u64) << 32 | low as u64;
                }
            }
            match valid(&name, length) {
                Ok((cached, noncached)) => {
                    files.valid_cached += cached;
                    files.valid += noncached;
                }
                Err(why) => {
                    if failed.0 == 0 {
                        failed.1 = why;
                    }
                    failed.0 += 1;
                }
            }
        }
        files.failed = (failed.0 > 0).then(|| format!("{} not asked: {}", failed.0, failed.1));
        Some(files)
    }

    /// How much of a file of `length` is valid data, cached and not, by `FSCTL_QUERY_FILE_REGIONS`. Opened only
    /// to read its attributes, sharing everything, so Steam's own use of it is not disturbed.
    fn valid(name: &[u16], length: u64) -> Result<(u64, u64), String> {
        // SAFETY: the handle is checked and closed; the buffers are sized as passed.
        unsafe {
            let share = FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE;
            let file = CreateFileW(
                name.as_ptr(),
                FILE_READ_ATTRIBUTES,
                share,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                std::ptr::null_mut(),
            );
            if file == INVALID_HANDLE_VALUE {
                return Err(format!("open failed, error {}", GetLastError()));
            }
            let (mut cached, mut noncached, mut from) = (0u64, 0u64, 0u64);
            let mut result = Ok(());
            while from < length {
                let input = FILE_REGION_INPUT {
                    FileOffset: from as i64,
                    Length: (length - from) as i64,
                    DesiredUsage: FILE_REGION_USAGE_VALID_CACHED_DATA | FILE_REGION_USAGE_VALID_NONCACHED_DATA,
                };
                // Room for 64 regions after the header.
                const ROOM: usize = 64;
                let size =
                    std::mem::size_of::<FILE_REGION_OUTPUT>() + (ROOM - 1) * std::mem::size_of::<FILE_REGION_INFO>();
                let mut buffer = vec![0u64; size.div_ceil(8)];
                let mut returned = 0u32;
                let ok = DeviceIoControl(
                    file,
                    FSCTL_QUERY_FILE_REGIONS,
                    (&input as *const FILE_REGION_INPUT).cast(),
                    std::mem::size_of::<FILE_REGION_INPUT>() as u32,
                    buffer.as_mut_ptr().cast(),
                    (buffer.len() * 8) as u32,
                    &mut returned,
                    std::ptr::null_mut(),
                );
                if ok == 0 {
                    result = Err(format!("FSCTL_QUERY_FILE_REGIONS failed, error {}", GetLastError()));
                    break;
                }
                let output = &*(buffer.as_ptr() as *const FILE_REGION_OUTPUT);
                let count = (output.RegionEntryCount as usize).min(ROOM);
                let regions = std::slice::from_raw_parts(output.Region.as_ptr(), count);
                let mut end = from;
                for region in regions {
                    let len = region.Length.max(0) as u64;
                    if region.Usage & FILE_REGION_USAGE_VALID_CACHED_DATA != 0 {
                        cached += len;
                    }
                    if region.Usage & FILE_REGION_USAGE_VALID_NONCACHED_DATA != 0 {
                        noncached += len;
                    }
                    end = end.max(region.FileOffset.max(0) as u64 + len);
                }
                // All of it said, or no further to go.
                if output.TotalRegionEntryCount as usize <= count || end <= from {
                    break;
                }
                from = end;
            }
            CloseHandle(file);
            result.map(|()| (cached, noncached))
        }
    }
}
