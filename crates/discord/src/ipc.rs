//! The real transport: Discord's pipe, `\\.\pipe\discord-ipc-0` to `-9` on Windows (Stable, PTB and Canary each take
//! the next free one), a Unix socket of the same name elsewhere; or, in a test, the one pipe a flag names
//! (`--discord-pipe`), so the Dashboard never reaches the real Discord there. A thread reads each connection and
//! hands its frames to the loop over a channel, waking it; the loop gives them to the [`Client`](crate::Client).
use std::io;
use std::sync::mpsc::Sender;
use std::sync::Arc;

use crate::client::Transport;
use crate::frame::{encode, Decoded, Decoder, Opcode};

/// What the loop is woken for.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// Something for the loop from another thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Incoming {
    /// A whole frame from the connection `generation`.
    Frame(u64, Opcode, String),
    /// The connection `generation` ended by itself: Discord closed it, or it broke.
    Closed(u64),
    /// The token endpoint's answer: its status and body, or 0 and what went wrong.
    Token(u16, String),
}

/// One connection's two ends: read on its own thread, written from the loop, shut from either.
pub trait Stream: Send + Sync {
    fn read(&self, buf: &mut [u8]) -> io::Result<usize>;
    fn write_all(&self, buf: &[u8]) -> io::Result<()>;
    /// Ends it: a read waiting on it returns.
    fn shut(&self);
}

/// The pipes Discord listens on, in the order they are tried.
pub fn discord_names() -> Vec<String> {
    (0..10).map(|n| format!("discord-ipc-{n}")).collect()
}

#[cfg(unix)]
mod unix {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::path::PathBuf;

    use super::Stream;

    /// Where a pipe of this name is: a path as it is, else a socket in the folder Discord uses,
    /// `$XDG_RUNTIME_DIR` (or the temporary folder).
    pub fn socket_path(name: &str) -> PathBuf {
        if name.contains('/') {
            return PathBuf::from(name);
        }
        let dir = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
            .iter()
            .find_map(|var| std::env::var_os(var).filter(|dir| !dir.is_empty()))
            .map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
        dir.join(name)
    }

    impl Stream for UnixStream {
        fn read(&self, buf: &mut [u8]) -> std::io::Result<usize> {
            Read::read(&mut &*self, buf)
        }

        fn write_all(&self, buf: &[u8]) -> std::io::Result<()> {
            Write::write_all(&mut &*self, buf)
        }

        fn shut(&self) {
            let _ = self.shutdown(std::net::Shutdown::Both);
        }
    }

    pub fn connect(name: &str) -> std::io::Result<UnixStream> {
        UnixStream::connect(socket_path(name))
    }
}

#[cfg(unix)]
pub use unix::socket_path;

/// The client's end of the pipe `name`, or an error at once where nobody listens on it.
pub fn connect(name: &str) -> io::Result<Arc<dyn Stream>> {
    #[cfg(windows)]
    return Ok(Arc::new(crate::win::Pipe::connect(name)?));
    #[cfg(unix)]
    return Ok(Arc::new(unix::connect(name)?));
    #[cfg(not(any(windows, unix)))]
    Err(io::Error::new(io::ErrorKind::Unsupported, format!("no pipes here for {name}")))
}

/// Reads `stream` until it ends, handing each whole frame to `deliver`. Ends on a broken stream too.
pub fn read_frames(stream: &dyn Stream, mut deliver: impl FnMut(Opcode, String)) -> Option<String> {
    let mut decoder = Decoder::default();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let read = match stream.read(&mut buf) {
            Ok(0) => return None,
            Ok(read) => read,
            Err(e) => return Some(e.to_string()),
        };
        decoder.push(&buf[..read]);
        while let Some(decoded) = decoder.take_frame() {
            match decoded {
                Decoded::Frame(opcode, json) => deliver(opcode, json),
                Decoded::Broken(what) => {
                    stream.shut();
                    return Some(format!("Discord sent {what}"));
                }
            }
        }
    }
}

/// Discord's pipe, or the one a test names.
pub struct Pipe {
    names: Vec<String>,
    incoming: Sender<Incoming>,
    wake: Wake,
    /// The connection open now, and its generation: what its reader hands over is told apart from an earlier one's.
    current: Option<(u64, Arc<dyn Stream>)>,
    generation: u64,
}

impl Pipe {
    /// Tries `names` in order, handing what comes to `incoming` and waking the loop with `wake`.
    pub fn new(names: Vec<String>, incoming: Sender<Incoming>, wake: Wake) -> Self {
        Self { names, incoming, wake, current: None, generation: 0 }
    }

    /// Whether what came from connection `generation` is the open connection's.
    pub fn is_current(&self, generation: u64) -> bool {
        self.current.as_ref().is_some_and(|(open, _)| *open == generation)
    }

    /// The connection `generation` has ended by itself: it is let go of.
    pub fn ended(&mut self, generation: u64) {
        if self.is_current(generation) {
            self.current = None;
        }
    }
}

impl Transport for Pipe {
    fn open(&mut self) -> bool {
        self.close();
        for name in &self.names {
            let Ok(stream) = connect(name) else { continue };
            self.generation += 1;
            let generation = self.generation;
            let (reader, incoming, wake) = (stream.clone(), self.incoming.clone(), self.wake.clone());
            let spawned = std::thread::Builder::new().name("discord pipe".into()).spawn(move || {
                let why = read_frames(&*reader, |opcode, json| {
                    let _ = incoming.send(Incoming::Frame(generation, opcode, json));
                    wake();
                });
                if let Some(why) = why {
                    eprintln!("discord: the pipe broke: {why}");
                }
                let _ = incoming.send(Incoming::Closed(generation));
                wake();
            });
            if spawned.is_err() {
                stream.shut();
                continue;
            }
            eprintln!("discord: pipe {name} open");
            self.current = Some((generation, stream));
            return true;
        }
        false
    }

    fn send(&mut self, opcode: Opcode, json: &str) {
        let Some((generation, stream)) = &self.current else { return };
        if let Err(e) = stream.write_all(&encode(opcode, json)) {
            // The pipe broke between Discord's last frame and this one: it ends as if Discord had closed it.
            eprintln!("discord: couldn't send a {opcode:?} frame: {e}");
            stream.shut();
            let _ = self.incoming.send(Incoming::Closed(*generation));
            (self.wake)();
        }
    }

    fn close(&mut self) {
        if let Some((_, stream)) = self.current.take() {
            stream.shut();
        }
    }
}

impl Drop for Pipe {
    /// The connection ends with the Dashboard's hold on it, its reader thread with it.
    fn drop(&mut self) {
        self.close();
    }
}
