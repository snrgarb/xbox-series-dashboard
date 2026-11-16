//! What only Windows can say of packages: which are registered for this user and where (the package manager), what
//! an `ms-resource:` name reads as, and which processes run as a package.
use std::path::PathBuf;

use anyhow::Result;
use windows::core::HSTRING;
use windows::Management::Deployment::PackageManager;
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_SUCCESS, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::Packaging::Appx::GetPackageFamilyName;
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
use windows_sys::Win32::UI::Shell::SHLoadIndirectString;

use crate::Registered;

/// Every package registered for this user that could be a game: frameworks and resource packages left out.
pub fn packages() -> Result<Vec<Registered>> {
    let manager = PackageManager::new()?;
    // An empty security id is the user this runs as; no administrator is needed for one's own.
    let mut found = Vec::new();
    for package in manager.FindPackagesByUserSecurityId(&HSTRING::new())? {
        if package.IsFramework().unwrap_or(true) || package.IsResourcePackage().unwrap_or(true) {
            continue;
        }
        // A package being installed or broken has no path to give.
        let (Ok(id), Ok(path)) = (package.Id(), package.InstalledPath()) else { continue };
        let (Ok(family_name), Ok(full_name)) = (id.FamilyName(), id.FullName()) else { continue };
        found.push(Registered {
            family_name: family_name.to_string(),
            full_name: full_name.to_string(),
            root: PathBuf::from(path.to_string()),
        });
    }
    Ok(found)
}

/// What an `ms-resource:` name in the package `full_name` (whose identity's name is `name`) reads as, from the
/// package's own resources: as Playnite builds the reference Windows looks up.
pub fn resource(full_name: &str, name: &str, reference: &str) -> Option<String> {
    let reference = reference.trim();
    let key = reference.get("ms-resource:".len()..)?;
    let uri = if reference.to_ascii_lowercase().starts_with("ms-resource://") {
        reference.to_string()
    } else if key.contains('/') {
        format!("ms-resource://{name}/{}", key.trim_start_matches('/'))
    } else {
        format!("ms-resource://{name}/resources/{key}")
    };
    let source: Vec<u16> = format!("@{{{full_name}? {uri}}}").encode_utf16().chain([0]).collect();
    let mut out = [0u16; 512];
    let result = unsafe { SHLoadIndirectString(source.as_ptr(), out.as_mut_ptr(), out.len() as u32, std::ptr::null()) };
    if result < 0 {
        return None;
    }
    let len = out.iter().position(|&c| c == 0).unwrap_or(out.len());
    let text = String::from_utf16_lossy(&out[..len]).trim().to_string();
    (!text.is_empty() && !text.to_ascii_lowercase().starts_with("ms-resource:")).then_some(text)
}

/// Whether any process of this session runs as the package family `family_name`: the game, and the helper that
/// starts it, are both in its package (docs/research/stores.md, "Running and ended").
pub fn running(family_name: &str) -> bool {
    let mut session = 0u32;
    if unsafe { ProcessIdToSessionId(std::process::id(), &mut session) } == 0 {
        return false;
    }
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return false;
    }
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
    let mut found = false;
    let mut more = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
    while more && !found {
        found = family_of(entry.th32ProcessID, session).is_some_and(|family| family.eq_ignore_ascii_case(family_name));
        more = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
    }
    unsafe { CloseHandle(snapshot) };
    found
}

/// The package family a process of `session` runs as; None for an ordinary program, or one of another session.
fn family_of(pid: u32, session: u32) -> Option<String> {
    let mut its = 0u32;
    if pid == 0 || unsafe { ProcessIdToSessionId(pid, &mut its) } == 0 || its != session {
        return None;
    }
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return None;
    }
    let mut name = [0u16; 256];
    let mut len = name.len() as u32;
    let result = unsafe { GetPackageFamilyName(process, &mut len, name.as_mut_ptr()) };
    unsafe { CloseHandle(process) };
    // APPMODEL_ERROR_NO_PACKAGE for an ordinary program.
    if result != ERROR_SUCCESS {
        return None;
    }
    let len = name.iter().position(|&c| c == 0).unwrap_or(name.len());
    Some(String::from_utf16_lossy(&name[..len]))
}
