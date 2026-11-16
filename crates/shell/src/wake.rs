//! Waking from sleep: Windows says the machine has resumed, and Who's playing today? asks again
//! (docs/spec/start-up.md, section 8). It is behind a trait, so that the Dashboard is told the same way however
//! it heard, and a test can tell it without sleeping the machine.
//!
//! On Windows the system's own word is had with `PowerRegisterSuspendResumeNotification`, a callback on a thread of
//! the system's, which needs no window (`WM_POWERBROADCAST` reaches only top-level windows, and winit's hides it).
//! The callback marks the wake and wakes the loop, which asks `Wakes::woke` each turn. A Dashboard started with
//! `--allow-toast-posts` also takes a pretended wake from another program (`standin wake`, a `WM_COPYDATA` to the
//! Toast strip's window: `model::post::WAKE_ID`), which goes the same way.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// What tells the Dashboard that the machine has woken from sleep.
pub trait Wakes {
    /// Whether the machine has woken since this was last asked.
    fn woke(&mut self) -> bool;
}

/// Set when the machine has resumed, or a wake was pretended, until it is taken.
static WOKE: AtomicBool = AtomicBool::new(false);
/// Wakes the Dashboard's loop, from whatever thread heard.
static WAKER: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

/// Marks a wake, and wakes the loop to hear of it.
fn mark(how: &str) {
    eprintln!("wake: {how} at {:.0} ms", crate::since_start());
    WOKE.store(true, Ordering::Relaxed);
    if let Some(wake) = WAKER.get() {
        wake();
    }
}

/// A wake pretended by another program: what `standin wake` posts to a Dashboard that takes posts.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn pretend() {
    mark("woken by a post, as from sleep");
}

/// The system's word. Made once; anywhere but Windows it never hears anything.
pub struct SystemWakes {
    #[cfg(windows)]
    _registration: Option<win::Registration>,
}

impl SystemWakes {
    /// Listens for the machine resuming from sleep; `wake` is called, on whatever thread heard, as it does.
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        let _ = WAKER.set(Box::new(wake));
        Self {
            #[cfg(windows)]
            _registration: win::Registration::new(),
        }
    }
}

impl Wakes for SystemWakes {
    fn woke(&mut self) -> bool {
        WOKE.swap(false, Ordering::Relaxed)
    }
}

/// A test's: it wakes when the test says.
#[derive(Default)]
pub struct FakeWakes {
    pub pending: bool,
}

impl Wakes for FakeWakes {
    fn woke(&mut self) -> bool {
        std::mem::take(&mut self.pending)
    }
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;
    use std::ptr::null_mut;

    use windows_sys::Win32::Foundation::{ERROR_SUCCESS, HANDLE};
    use windows_sys::Win32::System::Power::{
        PowerRegisterSuspendResumeNotification, PowerUnregisterSuspendResumeNotification,
        DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS, HPOWERNOTIFY,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DEVICE_NOTIFY_CALLBACK, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND,
    };

    /// The callback's registration, given up as it is dropped.
    pub struct Registration {
        handle: HPOWERNOTIFY,
        /// Where the system finds the callback: it must outlive the registration.
        _parameters: Box<DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS>,
    }

    /// Called by the system on a thread of its own. A resume is told twice when someone is there to see it
    /// (automatic, then the one the user woke); the first is enough, and a second wake while Who's playing today?
    /// already asks does nothing.
    unsafe extern "system" fn callback(_: *const c_void, kind: u32, _: *const c_void) -> u32 {
        if kind == PBT_APMRESUMEAUTOMATIC {
            super::mark("the machine woke from sleep");
        } else if kind == PBT_APMRESUMESUSPEND {
            eprintln!("wake: the user is back at {:.0} ms", crate::since_start());
        }
        0
    }

    impl Registration {
        pub fn new() -> Option<Self> {
            let mut parameters =
                Box::new(DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS { Callback: Some(callback), Context: null_mut() });
            let mut handle: *mut c_void = null_mut();
            let recipient = &mut *parameters as *mut DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS as HANDLE;
            let error =
                unsafe { PowerRegisterSuspendResumeNotification(DEVICE_NOTIFY_CALLBACK, recipient, &mut handle) };
            if error != ERROR_SUCCESS {
                eprintln!("wake: can't hear of waking from sleep (error {error})");
                return None;
            }
            Some(Self { handle: handle as HPOWERNOTIFY, _parameters: parameters })
        }
    }

    impl Drop for Registration {
        fn drop(&mut self) {
            unsafe { PowerUnregisterSuspendResumeNotification(self.handle) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wake_is_heard_once() {
        let mut fake = FakeWakes { pending: true };
        assert!(fake.woke());
        assert!(!fake.woke());
    }
}
