//! Home's window kept in front of a store's own client while the Dashboard drives that client by script (#245).
//! Steam's install wizard (`source_steam::client`) has Steam open its Install dialog, where it shows its storage
//! choice and its licence agreements, and bring its main window up over the Dashboard; before this, Steam's windows
//! were put back after each step, so they were seen for a moment, and the dialog for as long as the Dashboard's own
//! prompt was up. Now none of the client's windows is seen over the Dashboard at all:
//!
//! - **Home's window is made topmost** before the first script reaches the client (`model::Opener::keep_front`), so
//!   whatever the client shows comes up beneath it. The Dashboard's own topmost windows (the Overlay, the Toast strip)
//!   are put back above it at once, and each puts itself above it again as before: the Overlay whenever it is shown
//!   or raised, the Toast strip at each frame (`keep_above`).
//! - **The foreground is locked** (`LockSetForegroundWindow`) while Home has it, so the client is refused it, and
//!   with it the keyboard. The pad is read through XInput whoever has the foreground, but a window of Steam's with the
//!   foreground would answer the pad too (Steam's own controller navigation): its A could press the dialog's buttons.
//! - **A look every [`LOOK_EVERY`]**, on a thread of its own: a window of the client's (a program under its folder)
//!   that has the foreground all the same is taken back from at once (`take_foreground`) and the lock put on again;
//!   one of the client's above Home's window (one the client made topmost itself) has Home's put above it again.
//!   Each is a `front:` line in the log, with the window, so a run on the TV PC says whether Steam ever tried.
//!
//! It is kept only while Home's window is in front of every window not the Dashboard's (never over a game that runs,
//! nor over a window the user has in front), and let go when what kept it drops it, whichever way the wizard ended
//! (done, declined, refused, cancelled, failed, timed out, Steam gone: the installer drops it once the client's
//! windows are put back), or as soon as the Dashboard opens anything (a game launching, Steam's own install dialog
//! asked for, an app: [`let_go`], from `SystemAddresses::open`), which is the user's to see. Let go, Home's window is
//! no longer topmost and stays where it is, above every ordinary window, and the foreground is unlocked. Several
//! installs keeping it at once share one hold, let go with the last.
//!
//! Every Windows call is made on the hold's own thread, never on the caller's: the Dashboard's window thread may let
//! go (a launch) while the hold's thread is asking that same window to move, and neither waits for the other. Only
//! [`keep`] waits, on the installer's own thread, for Home's window to be topmost before the client is asked.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How often the hold looks at the foreground and at what is above Home's window.
pub const LOOK_EVERY: Duration = Duration::from_millis(15);

/// Home's window kept in front for as long as this is kept: dropped, the hold lets go once nothing else keeps it.
pub struct Kept {
    on: Arc<On>,
}

impl Drop for Kept {
    fn drop(&mut self) {
        let mut hold = HOLD.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.on.kept.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.on.let_go("the install is over");
            if hold.as_ref().is_some_and(|on| Arc::ptr_eq(on, &self.on)) {
                *hold = None;
            }
        }
    }
}

/// A hold, from [`keep`] until it is let go.
struct On {
    /// Home's window.
    home: isize,
    /// What it was kept for, as the log says it.
    what: String,
    /// The folders of the clients it is kept from: their programs' windows are the ones watched.
    clients: Mutex<Vec<PathBuf>>,
    /// Until it is let go.
    live: AtomicBool,
    /// Why it was let go, for the log.
    why: Mutex<String>,
    /// How many [`Kept`] hold it.
    kept: AtomicUsize,
}

impl On {
    fn let_go(&self, why: &str) {
        if self.live.swap(false, Ordering::SeqCst) {
            *self.why.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = why.to_string();
        }
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    fn live(&self) -> bool {
        self.live.load(Ordering::SeqCst)
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    fn clients(&self) -> Vec<PathBuf> {
        self.clients.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
    }
}

/// The hold on now, if any.
static HOLD: Mutex<Option<Arc<On>>> = Mutex::new(None);
/// Holds being made, one at a time.
static STARTING: Mutex<()> = Mutex::new(());

fn log(line: &str) {
    eprintln!("front: {line} at {:.0} ms", crate::since_start());
}

/// Keeps `home`, Home's window, in front of the windows of the client whose programs are under `client` for as long
/// as what it gives is kept: Home's window topmost, the foreground locked, and taken back from the client where it
/// gets it all the same (the module's doc). Waits until Home's window is topmost. None where it is not kept: Home's
/// window is not in front of every window not the Dashboard's (a game runs), or anywhere but Windows. `what` is what
/// is under way, for the log.
pub fn keep(home: isize, client: &Path, what: &str) -> Option<Kept> {
    let _starting = STARTING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    {
        let hold = HOLD.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(on) = hold.as_ref().filter(|on| on.live() && on.home == home) {
            on.kept.fetch_add(1, Ordering::SeqCst);
            on.clients.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(client.to_path_buf());
            log(&format!("Home's window kept in front for {what} as well, as it is for {}", on.what));
            return Some(Kept { on: on.clone() });
        }
    }
    let on = Arc::new(On {
        home,
        what: what.to_string(),
        clients: Mutex::new(vec![client.to_path_buf()]),
        live: AtomicBool::new(true),
        why: Mutex::new(String::new()),
        kept: AtomicUsize::new(1),
    });
    // On before it is made, so that a let-go meanwhile lets go of it.
    *HOLD.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(on.clone());
    let kept = start(on.clone());
    if !kept {
        on.let_go("not kept");
        let mut hold = HOLD.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if hold.as_ref().is_some_and(|held| Arc::ptr_eq(held, &on)) {
            *hold = None;
        }
        return None;
    }
    Some(Kept { on })
}

/// Lets go of the hold on now, if any, whatever still keeps it: the Dashboard opens something the user is to see
/// (`why`, for the log).
pub fn let_go(why: &str) {
    let on = HOLD.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take();
    if let Some(on) = on {
        on.let_go(why);
    }
}

/// Starts the hold's thread and waits for Home's window to be topmost; whether it was kept.
#[cfg(windows)]
fn start(on: Arc<On>) -> bool {
    let (told, told_of) = std::sync::mpsc::channel();
    let spawned = std::thread::Builder::new().name("front".into()).spawn(move || win::hold(&on, &told));
    if let Err(why) = spawned {
        log(&format!("not kept: no thread for it ({why})"));
        return false;
    }
    told_of.recv().unwrap_or(false)
}

#[cfg(not(windows))]
fn start(_on: Arc<On>) -> bool {
    false
}

#[cfg(windows)]
mod win {
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::path::PathBuf;
    use std::sync::mpsc::Sender;
    use std::time::Instant;

    use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT};
    use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows_sys::Win32::Graphics::Gdi::IntersectRect;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetForegroundWindow, GetWindow, GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId,
        IsWindow, IsWindowVisible, LockSetForegroundWindow, SetWindowPos, GWL_EXSTYLE, GW_HWNDPREV, HWND_NOTOPMOST,
        HWND_TOPMOST, LSFW_LOCK, LSFW_UNLOCK, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        WS_EX_TOPMOST,
    };

    use super::{log, On, LOOK_EVERY};

    /// How many windows above Home's are looked at, at most: the topmost ones (the taskbar, the Dashboard's own)
    /// and whatever is in front of it.
    const LOOK_ABOVE: usize = 256;

    /// The hold, on its own thread: Home's window made topmost and the foreground locked, `told` whether it was; a
    /// look every [`LOOK_EVERY`] until it is let go; then Home's window a window among others again.
    pub fn hold(on: &On, told: &Sender<bool>) {
        let home = on.home as HWND;
        let since = Instant::now();
        let kept = unsafe { begin(on, home) };
        let _ = told.send(kept);
        if !kept {
            return;
        }
        let mut watch = Watch { known: HashMap::new(), over: Vec::new(), taken: 0 };
        while on.live() {
            std::thread::sleep(LOOK_EVERY);
            if !on.live() || unsafe { IsWindow(home) } == 0 {
                break;
            }
            unsafe { watch.look(on, home, since) };
        }
        unsafe { end(on, home, since, watch.taken) };
    }

    /// What the looks keep: whether each process seen is the client's, the windows of the client's already said to
    /// be above Home's, and how many times the foreground was taken back.
    struct Watch {
        known: HashMap<u32, bool>,
        over: Vec<isize>,
        taken: u32,
    }

    impl Watch {
        fn clients(&mut self, on: &On, pid: u32) -> bool {
            if pid == std::process::id() || pid == 0 {
                return false;
            }
            if let Some(&known) = self.known.get(&pid) {
                return known;
            }
            let folders = on.clients();
            let theirs =
                program(pid).is_some_and(|path| folders.iter().any(|folder| source_steam::under(folder, &path)));
            self.known.insert(pid, theirs);
            theirs
        }

        /// One look: the foreground taken back from the client's window that has it; Home's window put above one of
        /// the client's that came above it.
        unsafe fn look(&mut self, on: &On, home: HWND, since: Instant) {
            let front = GetForegroundWindow();
            if !front.is_null() && front != home && self.clients(on, pid(front)) {
                let took = crate::take_foreground(home);
                let locked = LockSetForegroundWindow(LSFW_LOCK) != 0;
                self.taken += 1;
                let raised = raise_ours(home, SWP_ASYNCWINDOWPOS);
                log(&format!(
                    "{} of the client's took the foreground {} ms into {}: taken back for Home's window: {took} \
                     (the foreground {}), {raised} window(s) of the Dashboard's put back above it",
                    crate::describe(front),
                    since.elapsed().as_millis(),
                    on.what,
                    if locked { "locked again" } else { "could not be locked again" }
                ));
            }
            let mut above = home;
            for _ in 0..LOOK_ABOVE {
                above = GetWindow(above, GW_HWNDPREV);
                if above.is_null() {
                    break;
                }
                if IsWindowVisible(above) == 0 || !self.clients(on, pid(above)) {
                    continue;
                }
                let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS;
                SetWindowPos(home, HWND_TOPMOST, 0, 0, 0, 0, flags);
                raise_ours(home, SWP_ASYNCWINDOWPOS);
                if !self.over.contains(&(above as isize)) {
                    self.over.push(above as isize);
                    log(&format!(
                        "{} of the client's came above Home's window {} ms into {}: Home's put above it again",
                        crate::describe(above),
                        since.elapsed().as_millis(),
                        on.what
                    ));
                }
                break;
            }
        }
    }

    /// Home's window made topmost, the Dashboard's own topmost windows put back above it, and the foreground locked;
    /// not where a window not the Dashboard's is in front of Home's.
    unsafe fn begin(on: &On, home: HWND) -> bool {
        if home.is_null() || IsWindow(home) == 0 {
            log(&format!("Home's window not kept in front for {}: there is none", on.what));
            return false;
        }
        // The window that was in front as the install was asked, which the opener takes for Home's: topmost already,
        // it is the Overlay's (the Guide was open), and Home's own is not known here.
        if topmost(home) {
            log(&format!(
                "Home's window not kept in front for {}: the window in front as it was asked, {}, is topmost already",
                on.what,
                crate::describe(home)
            ));
            return false;
        }
        if let Some(over) = in_front_of(home) {
            log(&format!(
                "Home's window not kept in front for {}: {} is in front of it",
                on.what,
                crate::describe(over)
            ));
            return false;
        }
        SetWindowPos(home, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        let raised = raise_ours(home, 0);
        let locked = LockSetForegroundWindow(LSFW_LOCK) != 0;
        log(&format!(
            "Home's window kept in front for {}: topmost {}, the foreground {} ({} has it), {raised} window(s) of the \
             Dashboard's put back above it",
            on.what,
            topmost(home),
            if locked { "locked" } else { "could not be locked" },
            crate::describe(GetForegroundWindow())
        ));
        true
    }

    /// Home's window no longer topmost, where it is, and the foreground unlocked.
    unsafe fn end(on: &On, home: HWND, since: Instant, taken: u32) {
        let window = IsWindow(home) != 0;
        if window && topmost(home) {
            SetWindowPos(home, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
        let unlocked = LockSetForegroundWindow(LSFW_UNLOCK) != 0;
        let why = on.why.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone();
        log(&format!(
            "Home's window let go after {} ms ({why}): topmost {}, the foreground {}; {} has it; taken back from the \
             client {taken} time(s)",
            since.elapsed().as_millis(),
            window && topmost(home),
            if unlocked { "unlocked" } else { "not unlocked (no longer ours to unlock)" },
            crate::describe(GetForegroundWindow())
        ));
    }

    unsafe fn topmost(window: HWND) -> bool {
        GetWindowLongPtrW(window, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0
    }

    unsafe fn pid(window: HWND) -> u32 {
        let mut pid = 0u32;
        GetWindowThreadProcessId(window, &mut pid);
        pid
    }

    /// A window not the Dashboard's, nor topmost, shown and not cloaked, over some of Home's window and above it:
    /// what the user has in front (a game, a program).
    unsafe fn in_front_of(home: HWND) -> Option<HWND> {
        let mut area: RECT = std::mem::zeroed();
        GetWindowRect(home, &mut area);
        let mut above = home;
        for _ in 0..LOOK_ABOVE {
            above = GetWindow(above, GW_HWNDPREV);
            if above.is_null() {
                return None;
            }
            if IsWindowVisible(above) == 0 || topmost(above) || pid(above) == std::process::id() {
                continue;
            }
            let mut cloaked = 0u32;
            DwmGetWindowAttribute(above, DWMWA_CLOAKED as u32, &mut cloaked as *mut u32 as *mut c_void, 4);
            let (mut rect, mut both): (RECT, RECT) = (std::mem::zeroed(), std::mem::zeroed());
            GetWindowRect(above, &mut rect);
            if cloaked == 0 && IntersectRect(&mut both, &rect, &area) != 0 {
                return Some(above);
            }
        }
        None
    }

    /// The Dashboard's own shown topmost windows but Home's (the Overlay, the Toast strip) put back above every
    /// other, in the order they were; how many.
    unsafe fn raise_ours(home: HWND, flags: u32) -> usize {
        let mut ours: Vec<isize> = vec![home as isize];
        EnumWindows(Some(each), &mut ours as *mut Vec<isize> as LPARAM);
        // Top to bottom as Windows gave them; raised bottom first, so that they keep their order.
        let raised: Vec<isize> = ours.split_off(1);
        for &window in raised.iter().rev() {
            let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | flags;
            SetWindowPos(window as HWND, HWND_TOPMOST, 0, 0, 0, 0, flags);
        }
        raised.len()
    }

    unsafe extern "system" fn each(window: HWND, param: LPARAM) -> i32 {
        let ours = &mut *(param as *mut Vec<isize>);
        if window as isize != ours[0]
            && IsWindowVisible(window) != 0
            && topmost(window)
            && pid(window) == std::process::id()
        {
            ours.push(window as isize);
        }
        1
    }

    fn program(pid: u32) -> Option<PathBuf> {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Anywhere but Windows nothing is kept, and letting go of nothing does nothing.
    #[test]
    fn nothing_is_kept_where_there_is_no_window_to_keep() {
        if cfg!(windows) {
            return;
        }
        assert!(keep(0, Path::new("/steam"), "install 620").is_none());
        assert!(HOLD.lock().unwrap().is_none());
        let_go("nothing");
    }

    /// Two installs keep one hold; it is let go with the last, and a let-go lets go whatever still keeps it.
    #[test]
    fn a_hold_kept_twice_is_let_go_with_the_last() {
        let on = Arc::new(On {
            home: 1,
            what: "install 620".into(),
            clients: Mutex::new(Vec::new()),
            live: AtomicBool::new(true),
            why: Mutex::new(String::new()),
            kept: AtomicUsize::new(2),
        });
        let (first, second) = (Kept { on: on.clone() }, Kept { on: on.clone() });
        drop(first);
        assert!(on.live());
        drop(second);
        assert!(!on.live());
        assert_eq!(*on.why.lock().unwrap(), "the install is over");
        let on = Arc::new(On { live: AtomicBool::new(true), kept: AtomicUsize::new(1), ..clone_of(&on) });
        on.let_go("a game launches");
        drop(Kept { on: on.clone() });
        assert_eq!(*on.why.lock().unwrap(), "a game launches", "the first reason stays");
    }

    fn clone_of(on: &On) -> On {
        On {
            home: on.home,
            what: on.what.clone(),
            clients: Mutex::new(on.clients()),
            live: AtomicBool::new(on.live()),
            why: Mutex::new(String::new()),
            kept: AtomicUsize::new(on.kept.load(Ordering::SeqCst)),
        }
    }
}
