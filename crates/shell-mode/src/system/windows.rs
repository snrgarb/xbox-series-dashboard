//! [`ThisPc`] on Windows: the registry through advapi32, tasks through schtasks.exe at the caller's own rights (a
//! logon task for the user registers without admin, as the Bundle found on the TV PC), files through std.
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteKeyValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE, KEY_WOW64_64KEY, REG_DWORD,
    REG_OPTION_NON_VOLATILE, REG_SZ,
};

use super::{files, Hive, Place, System, Task, ThisPc, Value};
use crate::{contract, task_xml};

/// CREATE_NO_WINDOW: schtasks and the probe show no console.
const NO_WINDOW: u32 = 0x0800_0000;

/// This process's user as its token has it, as a SID string (`S-1-5-21-...`), which Task Scheduler takes for a
/// `UserId`. Never from the environment: in an SSH session `USERDOMAIN` is `WORKGROUP`, which names no account.
pub fn token_user() -> Result<String, String> {
    use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, HANDLE};
    use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows_sys::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(format!("this process's token: {}", std::io::Error::last_os_error()));
        }
        let mut needed = 0u32;
        GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut needed);
        // u64s, so the TOKEN_USER at its start is aligned.
        let mut buffer = vec![0u64; (needed as usize).div_ceil(8).max(1)];
        let ok = GetTokenInformation(token, TokenUser, buffer.as_mut_ptr().cast(), needed, &mut needed) != 0;
        let failed = std::io::Error::last_os_error();
        CloseHandle(token);
        if !ok {
            return Err(format!("this process's user: {failed}"));
        }
        let user = &*(buffer.as_ptr() as *const TOKEN_USER);
        let mut text: windows_sys::core::PWSTR = std::ptr::null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut text) == 0 {
            return Err(format!("this user's SID as text: {}", std::io::Error::last_os_error()));
        }
        let length = (0..).take_while(|&i| *text.add(i) != 0).count();
        let sid = String::from_utf16_lossy(std::slice::from_raw_parts(text, length));
        LocalFree(text.cast());
        Ok(sid)
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

fn error(code: u32) -> String {
    std::io::Error::from_raw_os_error(code as i32).to_string()
}

fn root(hive: Hive) -> HKEY {
    match hive {
        Hive::User => HKEY_CURRENT_USER,
        Hive::Machine => HKEY_LOCAL_MACHINE,
    }
}

impl System for ThisPc {
    fn read(&self, place: &Place) -> Result<Option<Value>, String> {
        let (key, name) = (wide(&place.key), wide(&place.name));
        unsafe {
            let mut opened: HKEY = std::ptr::null_mut();
            match RegOpenKeyExW(root(place.hive), key.as_ptr(), 0, KEY_READ | KEY_WOW64_64KEY, &mut opened) {
                ERROR_SUCCESS => {}
                ERROR_FILE_NOT_FOUND => return Ok(None),
                code => return Err(format!("{place}: {}", error(code))),
            }
            let mut kind = 0;
            let mut size = 0u32;
            let mut data = Vec::new();
            let result = loop {
                let code = RegQueryValueExW(
                    opened,
                    name.as_ptr(),
                    std::ptr::null(),
                    &mut kind,
                    if data.is_empty() { std::ptr::null_mut() } else { data.as_mut_ptr() },
                    &mut size,
                );
                match code {
                    ERROR_SUCCESS if !data.is_empty() || size == 0 => break Ok(()),
                    ERROR_SUCCESS | ERROR_MORE_DATA => data.resize(size.max(1) as usize, 0),
                    code => break Err(code),
                }
            };
            RegCloseKey(opened);
            match result {
                Ok(()) => {}
                Err(ERROR_FILE_NOT_FOUND) => return Ok(None),
                Err(code) => return Err(format!("{place}: {}", error(code))),
            }
            data.truncate(size as usize);
            Ok(Some(match kind {
                REG_SZ => {
                    let units: Vec<u16> =
                        data.as_chunks::<2>().0.iter().map(|pair| u16::from_le_bytes(*pair)).collect();
                    let end = units.iter().position(|&unit| unit == 0).unwrap_or(units.len());
                    Value::Text(String::from_utf16_lossy(&units[..end]))
                }
                REG_DWORD if data.len() == 4 => Value::Number(u32::from_le_bytes([data[0], data[1], data[2], data[3]])),
                kind => Value::Raw { kind, data },
            }))
        }
    }

    fn write(&mut self, place: &Place, value: Option<&Value>) -> Result<(), String> {
        // The one rule kept here whoever asks: nothing of the machine's own is ever written.
        if place.hive != Hive::User {
            return Err(format!("{place}: HKLM is never written"));
        }
        let (key, name) = (wide(&place.key), wide(&place.name));
        unsafe {
            let Some(value) = value else {
                return match RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) {
                    ERROR_SUCCESS | ERROR_FILE_NOT_FOUND => Ok(()),
                    code => Err(format!("{place}: {}", error(code))),
                };
            };
            let (kind, data): (u32, Vec<u8>) = match value {
                Value::Text(text) => (REG_SZ, wide(text).iter().flat_map(|unit| unit.to_le_bytes()).collect()),
                Value::Number(n) => (REG_DWORD, n.to_le_bytes().to_vec()),
                Value::Raw { kind, data } => (*kind, data.clone()),
            };
            let mut opened: HKEY = std::ptr::null_mut();
            let code = RegCreateKeyExW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                std::ptr::null(),
                &mut opened,
                std::ptr::null_mut(),
            );
            if code != ERROR_SUCCESS {
                return Err(format!("{place}: {}", error(code)));
            }
            let code = RegSetValueExW(opened, name.as_ptr(), 0, kind, data.as_ptr(), data.len() as u32);
            RegCloseKey(opened);
            if code != ERROR_SUCCESS {
                return Err(format!("{place}: {}", error(code)));
            }
            Ok(())
        }
    }

    fn task(&self, name: &str) -> Result<Option<Task>, String> {
        let (code, out, err) = schtasks(&["/Query", "/TN", name, "/XML"])?;
        if code == 0 {
            return Ok(Some(task_xml::read(&out)));
        }
        // schtasks fails the same way for a task that isn't there as for one it can't read, and says which only in
        // Windows' own language: its list of every task tells, by name, in any (the Bundle's UserTasks).
        let (listed, list, list_err) = schtasks(&["/Query", "/FO", "CSV", "/NH"])?;
        if listed != 0 {
            return Err(format!("Task {name}: {err}; listing the tasks: {list_err}"));
        }
        if task_xml::lists(&list, name) {
            return Err(format!("Task {name}: {err}"));
        }
        Ok(None)
    }

    fn set_task(&mut self, name: &str, task: Option<&Task>) -> Result<(), String> {
        let Some(task) = task else {
            if self.task(name)?.is_none() {
                return Ok(());
            }
            let (code, _, err) = schtasks(&["/Delete", "/TN", name, "/F"])?;
            return if code == 0 { Ok(()) } else { Err(format!("Task {name}: {err}")) };
        };
        // schtasks reads the XML from a file, as UTF-16, which is written in the data folder and removed after.
        let file = self.scratch.join(format!("{name}.xml"));
        let xml = task_xml::write(task, &self.user);
        let bytes: Vec<u8> = [0xFF, 0xFE].into_iter().chain(xml.encode_utf16().flat_map(|u| u.to_le_bytes())).collect();
        std::fs::create_dir_all(&self.scratch).map_err(|e| format!("{}: {e}", self.scratch.display()))?;
        std::fs::write(&file, bytes).map_err(|e| format!("{}: {e}", file.display()))?;
        let result = schtasks(&["/Create", "/TN", name, "/XML", &file.to_string_lossy(), "/F"]);
        let _ = std::fs::remove_file(&file);
        let (code, _, err) = result?;
        if code == 0 {
            Ok(())
        } else {
            Err(format!("Task {name}: {err}"))
        }
    }

    fn read_file(&self, path: &Path) -> Result<Option<String>, String> {
        files::read(path)
    }

    fn write_file(&mut self, path: &Path, text: &str) -> Result<(), String> {
        files::write(path, text)
    }

    fn rename_file(&mut self, from: &Path, to: &Path) -> Result<(), String> {
        std::fs::rename(from, to).map_err(|e| format!("{}: {e}", from.display()))
    }

    fn exists(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn probe(&self, exe: &Path) -> Result<(), String> {
        let mut child = Command::new(exe)
            .arg(contract::PROBE)
            .creation_flags(NO_WINDOW)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("{} didn't start: {e}", exe.display()))?;
        let began = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) if status.code().map(|c| c as u32) == Some(contract::PROBE_OK) => return Ok(()),
                Ok(Some(status)) => return Err(format!("{} {} ended with {status}", exe.display(), contract::PROBE)),
                Ok(None) if began.elapsed() > Duration::from_secs(20) => {
                    let _ = child.kill();
                    return Err(format!("{} {} didn't end within 20 s", exe.display(), contract::PROBE));
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(e) => return Err(format!("{}: {e}", exe.display())),
            }
        }
    }
}

/// Runs schtasks.exe from System32; its exit code, its output and its errors.
fn schtasks(arguments: &[&str]) -> Result<(i32, String, String), String> {
    let windows = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    let program = Path::new(&windows).join("System32").join("schtasks.exe");
    let output = Command::new(&program)
        .args(arguments)
        .creation_flags(NO_WINDOW)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("{}: {e}", program.display()))?;
    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).trim().to_string();
    Ok((output.status.code().unwrap_or(-1), text(&output.stdout), text(&output.stderr)))
}
