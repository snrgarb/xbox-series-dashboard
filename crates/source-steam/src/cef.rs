//! Steam's CEF debugger, the Bundle's first way of asking Steam (its `SteamDownloads.Cef`): the client opens it on
//! 127.0.0.1 (8080, else 8081) at each start while an empty file, `.cef-enable-remote-debugging`, is in its folder.
//! Script run there in its `SharedJSContext`, the page that runs without a window, reaches the client's own
//! `SteamClient` object: `SteamClient.Console.ExecCommand('app_install 620')` types the command into the client's
//! console, which starts the download with no Steam window and without Steam taking the foreground. Every way
//! through `steam.exe` raises a window of some kind (the Bundle's trials, #59 there; and #132 here, on the real
//! machine), so this is the way that never shows one.
//!
//! It is the user's to turn on, never the Dashboard's: while it is on, any program on the PC can drive Steam through
//! that port. The Bundle offers it only in its Debug menu. With it off, Steam is asked through `steam.exe`.
//!
//! The debugger speaks HTTP for its list of pages (`/json`) and the DevTools protocol over a WebSocket for each,
//! both written here over a plain TCP connection to the PC's own loopback: one request, one answer.
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::time::{Duration, Instant};

/// The empty file in Steam's folder that has the client open its debugger at each start.
pub const FILE: &str = ".cef-enable-remote-debugging";
/// The ports Steam's debugger answers on: the first free of these.
pub const PORTS: [u16; 2] = [8080, 8081];
/// The client's page that runs without a window.
pub const SHARED: &str = "SharedJSContext";
/// How long a port's list of pages is waited for: the debugger answers at once on the PC's own loopback when it is
/// open, so one that has not answered by then is taken as closed.
pub const PROBE: Duration = Duration::from_secs(2);
/// How long the script's answer is waited for, once it is sent.
pub const ANSWER: Duration = Duration::from_secs(30);

/// The file beside the stand-in Steam's `steam.exe` in which its debugger (`crate::debugger`) writes the port it
/// listens on, a port of its own: the stand-in never takes 8080 or 8081, where a real Steam's may be.
pub const STANDIN_PORT: &str = "standin-cef-port.txt";

/// Whether the Steam at `root` opens its debugger as it starts, as its folder now says. A Steam started before the
/// file was put there has it closed until it starts again; asking then finds no debugger and says so.
pub fn enabled(root: &Path) -> bool {
    root.join(FILE).is_file()
}

/// The ports the debugger of the Steam at `root` answers on: [`PORTS`] for a real Steam; for the stand-in Steam of
/// the end-to-end tests (`crate::fixture::STANDIN_CLIENT` in its folder) only the port its debugger wrote down
/// ([`STANDIN_PORT`]), none while it has written none. So a test's fake is never asked through a real Steam's
/// debugger, nor the real one through the fake's.
pub fn ports(root: &Path) -> Vec<u16> {
    if !root.join(crate::fixture::STANDIN_CLIENT).is_file() {
        return PORTS.to_vec();
    }
    let written = std::fs::read_to_string(root.join(STANDIN_PORT)).unwrap_or_default();
    written.trim().parse::<u16>().ok().filter(|&port| port != 0).into_iter().collect()
}

/// The client's console's command for what `steam.exe` is given (`crate::command`): `+app_install 620` is
/// `app_install 620`. None for arguments with no such command.
pub fn console_command(args: &[String]) -> Option<String> {
    let at = args.iter().position(|arg| arg.starts_with('+'))?;
    let command = args[at].trim_start_matches('+');
    let id: u32 = args.get(at + 1)?.parse().ok()?;
    (!command.is_empty() && command.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        .then(|| format!("{command} {id}"))
}

/// The script that types `command` into the client's console and says so.
pub fn console_script(command: &str) -> String {
    format!("SteamClient.Console.ExecCommand('{command}'); 'asked'")
}

/// What the script came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evaluated {
    /// Its value, as text.
    Value(String),
    /// It threw this.
    Threw(String),
    /// It gave nothing.
    Nothing,
}

/// Why it came to nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failed {
    /// Nothing reached Steam: no debugger answered, none had the page, or it would not take the connection.
    /// Another way of asking is safe.
    NotSent(String),
    /// The script was sent and the connection was lost before its answer: it may well have run, and asking again
    /// another way would ask twice.
    Lost(String),
}

impl std::fmt::Display for Failed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failed::NotSent(why) => write!(f, "{why}"),
            Failed::Lost(why) => write!(f, "lost after sending: {why}"),
        }
    }
}

/// Runs `expression` in the page titled `target` of the first debugger on 127.0.0.1's `ports` that has it, and
/// gives what it came to, waiting [`ANSWER`] at most for it.
pub fn evaluate(ports: &[u16], target: &str, expression: &str) -> Result<Evaluated, Failed> {
    evaluate_within(ports, target, expression, ANSWER)
}

/// [`evaluate`], waiting `answer` at most for the script's answer once it is sent.
pub fn evaluate_within(ports: &[u16], target: &str, expression: &str, answer: Duration) -> Result<Evaluated, Failed> {
    let mut tried = Vec::new();
    let found = ports.iter().find_map(|&port| match page(port, target) {
        Ok(url) => Some(url),
        Err(why) => {
            tried.push(format!("{port}: {why}"));
            None
        }
    });
    if ports.is_empty() {
        tried.push("no port to ask: the stand-in Steam's debugger has written none".into());
    }
    let Some(url) = found else { return Err(Failed::NotSent(tried.join("; "))) };
    run(&url, expression, answer)
}

/// The WebSocket address of the page titled `target` in the list a debugger at `port` gives.
fn page(port: u16, target: &str) -> Result<String, String> {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&address, PROBE).map_err(|e| e.to_string())?;
    stream.set_read_timeout(Some(PROBE)).map_err(|e| e.to_string())?;
    let request = format!("GET /json HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(stream);
    let headers = headers(&mut reader)?;
    if !headers.first().is_some_and(|status| status.split(' ').nth(1) == Some("200")) {
        return Err(format!("answered {:?}", headers.first().map_or("nothing", String::as_str)));
    }
    let header = |name: &str| {
        headers.iter().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.trim().eq_ignore_ascii_case(name).then(|| value.trim().to_string())
        })
    };
    let mut body = Vec::new();
    if header("transfer-encoding").is_some_and(|value| value.eq_ignore_ascii_case("chunked")) {
        loop {
            let mut size = String::new();
            reader.read_line(&mut size).map_err(|e| e.to_string())?;
            let size =
                usize::from_str_radix(size.trim().split(';').next().unwrap_or(""), 16).map_err(|e| e.to_string())?;
            if size == 0 {
                break;
            }
            let mut chunk = vec![0; size + 2];
            reader.read_exact(&mut chunk).map_err(|e| e.to_string())?;
            body.extend_from_slice(&chunk[..size]);
        }
    } else if let Some(length) = header("content-length").and_then(|value| value.parse::<usize>().ok()) {
        body.resize(length, 0);
        reader.read_exact(&mut body).map_err(|e| e.to_string())?;
    } else {
        reader.read_to_end(&mut body).map_err(|e| e.to_string())?;
    }
    let pages: serde_json::Value = serde_json::from_slice(&body).map_err(|_| "not a debugger's list of pages")?;
    let pages = pages.as_array().ok_or("not a debugger's list of pages")?;
    pages
        .iter()
        .filter(|page| page.get("title").and_then(|title| title.as_str()) == Some(target))
        .find_map(|page| page.get("webSocketDebuggerUrl")?.as_str().map(str::to_string))
        .ok_or_else(|| format!("no {target} among its {} pages", pages.len()))
}

/// An HTTP answer's status line and headers, up to the empty line.
pub(crate) fn headers(reader: &mut impl BufRead) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            return Err("closed before its headers ended".into());
        }
        let line = line.trim_end().to_string();
        if line.is_empty() {
            return Ok(lines);
        }
        lines.push(line);
    }
}

/// Runs the expression in the page at the WebSocket address `url` (`ws://127.0.0.1:8080/devtools/page/<id>`).
fn run(url: &str, expression: &str, answer_within: Duration) -> Result<Evaluated, Failed> {
    let not_sent = |why: String| Failed::NotSent(why);
    let rest = url.strip_prefix("ws://").ok_or_else(|| not_sent(format!("not a ws:// address: {url}")))?;
    let (host, path) = rest.split_once('/').map_or((rest, "/".to_string()), |(host, path)| (host, format!("/{path}")));
    let address: SocketAddr = match host.parse() {
        Ok(address) => address,
        // The debugger names itself by the address it was asked at, else as localhost.
        Err(_) => host
            .rsplit_once(':')
            .and_then(|(_, port)| port.parse::<u16>().ok())
            .map(|port| SocketAddr::from(([127, 0, 0, 1], port)))
            .ok_or_else(|| not_sent(format!("no port in {url}")))?,
    };
    if !address.ip().is_loopback() {
        return Err(not_sent(format!("{url} is not on this PC")));
    }
    let mut stream = TcpStream::connect_timeout(&address, PROBE).map_err(|e| not_sent(e.to_string()))?;
    stream.set_read_timeout(Some(PROBE)).map_err(|e| not_sent(e.to_string()))?;
    let key = base64(&nonce());
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).map_err(|e| not_sent(e.to_string()))?;
    let mut reader = BufReader::new(stream.try_clone().map_err(|e| not_sent(e.to_string()))?);
    let headers = headers(&mut reader).map_err(not_sent)?;
    if !headers.first().is_some_and(|status| status.split(' ').nth(1) == Some("101")) {
        return Err(not_sent(format!("refused the connection: {:?}", headers.first())));
    }

    // From here the script may run: what goes wrong is that it was lost.
    let lost = |why: String| Failed::Lost(why);
    let message = serde_json::json!({
        "id": 1,
        "method": "Runtime.evaluate",
        "params": { "expression": expression, "awaitPromise": true, "returnByValue": true },
    });
    stream.write_all(&frame(message.to_string().as_bytes())).map_err(|e| lost(e.to_string()))?;
    let until = Instant::now() + answer_within;
    loop {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(lost(format!("no answer in {} s", answer_within.as_secs())));
        }
        reader.get_ref().set_read_timeout(Some(left)).map_err(|e| lost(e.to_string()))?;
        let text = message_text(&mut reader).map_err(lost)?;
        let Ok(reply) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
        if reply.get("id").and_then(|id| id.as_u64()) != Some(1) {
            continue;
        }
        return Ok(answer(&reply));
    }
}

/// What a reply to `Runtime.evaluate` says the script came to.
fn answer(reply: &serde_json::Value) -> Evaluated {
    let result = &reply["result"];
    if let Some(thrown) = result.get("exceptionDetails") {
        let said = thrown["exception"]["description"].as_str().or(thrown["text"].as_str()).unwrap_or("something");
        return Evaluated::Threw(said.to_string());
    }
    match &result["result"]["value"] {
        serde_json::Value::Null => Evaluated::Nothing,
        serde_json::Value::String(text) => Evaluated::Value(text.clone()),
        value => Evaluated::Value(value.to_string()),
    }
}

/// A text frame from the client, masked as a client's must be.
fn frame(payload: &[u8]) -> Vec<u8> {
    let mut out = vec![0x81];
    match payload.len() {
        len if len < 126 => out.push(0x80 | len as u8),
        len if len <= u16::MAX as usize => {
            out.push(0x80 | 126);
            out.extend_from_slice(&(len as u16).to_be_bytes());
        }
        len => {
            out.push(0x80 | 127);
            out.extend_from_slice(&(len as u64).to_be_bytes());
        }
    }
    let mask: [u8; 4] = nonce()[..4].try_into().unwrap_or([1, 2, 3, 4]);
    out.extend_from_slice(&mask);
    out.extend(payload.iter().enumerate().map(|(i, byte)| byte ^ mask[i % 4]));
    out
}

/// A text frame from the server, unmasked as a server's is: what the stand-in debugger answers with.
pub(crate) fn server_frame(payload: &[u8]) -> Vec<u8> {
    let mut out = vec![0x81];
    match payload.len() {
        len if len < 126 => out.push(len as u8),
        len if len <= u16::MAX as usize => {
            out.push(126);
            out.extend_from_slice(&(len as u16).to_be_bytes());
        }
        len => {
            out.push(127);
            out.extend_from_slice(&(len as u64).to_be_bytes());
        }
    }
    out.extend_from_slice(payload);
    out
}

/// The next whole text message from the other end, masked or not, its fragments joined; pings and pongs passed
/// over.
pub(crate) fn message_text(reader: &mut impl Read) -> Result<String, String> {
    let mut message = Vec::new();
    loop {
        let mut head = [0u8; 2];
        reader.read_exact(&mut head).map_err(|e| e.to_string())?;
        let (fin, opcode, masked) = (head[0] & 0x80 != 0, head[0] & 0x0F, head[1] & 0x80 != 0);
        let len = match head[1] & 0x7F {
            126 => {
                let mut len = [0u8; 2];
                reader.read_exact(&mut len).map_err(|e| e.to_string())?;
                u16::from_be_bytes(len) as u64
            }
            127 => {
                let mut len = [0u8; 8];
                reader.read_exact(&mut len).map_err(|e| e.to_string())?;
                u64::from_be_bytes(len)
            }
            len => len as u64,
        };
        if len > 64 << 20 {
            return Err(format!("a message of {len} bytes"));
        }
        let mut mask = [0u8; 4];
        if masked {
            reader.read_exact(&mut mask).map_err(|e| e.to_string())?;
        }
        let mut payload = vec![0u8; len as usize];
        reader.read_exact(&mut payload).map_err(|e| e.to_string())?;
        if masked {
            payload.iter_mut().enumerate().for_each(|(i, byte)| *byte ^= mask[i % 4]);
        }
        match opcode {
            0x8 => return Err("the debugger closed the connection".into()),
            0x9 | 0xA => continue,
            _ => message.extend_from_slice(&payload),
        }
        if fin {
            return Ok(String::from_utf8_lossy(&message).into_owned());
        }
    }
}

/// Sixteen bytes that differ from one call to the next: a WebSocket key and mask, which need not be secret.
fn nonce() -> [u8; 16] {
    use std::hash::{BuildHasher, Hasher};
    let mut out = [0u8; 16];
    for half in out.chunks_mut(8) {
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u128(
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos(),
        );
        half.copy_from_slice(&hasher.finish().to_le_bytes());
    }
    out
}

/// Standard base64, with padding.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, &byte)| n | (byte as u32) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;

    use super::*;

    #[test]
    fn the_console_command_is_steam_exes_without_its_plus() {
        let args = |args: &[&str]| args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();
        assert_eq!(console_command(&args(&["-silent", "+app_install", "620"])).as_deref(), Some("app_install 620"));
        let uninstall = args(&["-silent", "-console", "+app_uninstall", "60"]);
        assert_eq!(console_command(&uninstall).as_deref(), Some("app_uninstall 60"));
        assert_eq!(console_command(&args(&["-silent", "+app_install", "x"])), None);
        assert_eq!(console_command(&args(&["+app_install');alert('", "1"])), None);
        assert_eq!(console_script("app_install 620"), "SteamClient.Console.ExecCommand('app_install 620'); 'asked'");
    }

    #[test]
    fn base64_is_the_standard_one() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    /// A stand-in debugger on a port of its own: its list of pages, then one WebSocket that answers the first
    /// `Runtime.evaluate` it is sent with `reply`, after an event of its own. Gives its port and what it was sent.
    fn debugger(reply: serde_json::Value) -> (u16, std::thread::JoinHandle<serde_json::Value>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            // The list of pages, chunked, as a server may send it.
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let request = headers(&mut reader).unwrap();
            assert_eq!(request[0], "GET /json HTTP/1.1");
            let pages = serde_json::json!([
                { "title": "Steam", "webSocketDebuggerUrl": format!("ws://127.0.0.1:{port}/devtools/page/MAIN") },
                { "title": SHARED, "webSocketDebuggerUrl": format!("ws://127.0.0.1:{port}/devtools/page/SHARED") },
            ])
            .to_string();
            let (first, second) = pages.split_at(10);
            let body = format!("{:x}\r\n{first}\r\n{:x}\r\n{second}\r\n0\r\n\r\n", first.len(), second.len());
            write!(stream, "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{body}").unwrap();
            drop(stream);

            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let request = headers(&mut reader).unwrap();
            assert_eq!(request[0], "GET /devtools/page/SHARED HTTP/1.1");
            assert!(request.iter().any(|line| line.eq_ignore_ascii_case("upgrade: websocket")));
            stream
                .write_all(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n")
                .unwrap();
            let sent: serde_json::Value = serde_json::from_str(&message_text(&mut reader).unwrap()).unwrap();
            let server = |text: &str| {
                let mut out = vec![0x81, 126];
                out.extend_from_slice(&(text.len() as u16).to_be_bytes());
                out.extend_from_slice(text.as_bytes());
                out
            };
            stream.write_all(&server(r#"{"method":"Runtime.executionContextCreated","params":{}}"#)).unwrap();
            stream.write_all(&server(&reply.to_string())).unwrap();
            sent
        });
        (port, handle)
    }

    #[test]
    fn a_script_runs_in_the_page_asked_for_and_its_value_comes_back() {
        let (port, server) =
            debugger(serde_json::json!({ "id": 1, "result": { "result": { "type": "string", "value": "asked" } } }));
        let script = console_script("app_install 620");
        assert_eq!(evaluate(&[port], SHARED, &script), Ok(Evaluated::Value("asked".into())));
        let sent = server.join().unwrap();
        assert_eq!(sent["method"], "Runtime.evaluate");
        assert_eq!(sent["params"]["expression"], script.as_str());
        assert_eq!(sent["params"]["awaitPromise"], true);
    }

    #[test]
    fn a_script_that_throws_says_what() {
        let thrown = serde_json::json!({ "id": 1, "result": { "result": {}, "exceptionDetails": {
            "text": "Uncaught", "exception": { "description": "ReferenceError: SteamClient is not defined" } } } });
        let (port, server) = debugger(thrown);
        let got = evaluate(&[port], SHARED, "SteamClient");
        assert_eq!(got, Ok(Evaluated::Threw("ReferenceError: SteamClient is not defined".into())));
        server.join().unwrap();
    }

    #[test]
    fn with_no_debugger_nothing_is_sent() {
        // A port nothing listens on: bound and let go.
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        assert!(matches!(evaluate(&[port], SHARED, "1"), Err(Failed::NotSent(_))));
        // Something else on the port: not a debugger.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let other = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            headers(&mut BufReader::new(stream.try_clone().unwrap())).unwrap();
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\n<html></html>").unwrap();
        });
        let got = evaluate(&[port], SHARED, "1");
        assert_eq!(got, Err(Failed::NotSent(format!("{port}: not a debugger's list of pages"))));
        other.join().unwrap();
    }

    #[test]
    fn the_file_in_steams_folder_says_whether_it_is_on() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!enabled(dir.path()));
        std::fs::write(dir.path().join(FILE), "").unwrap();
        assert!(enabled(dir.path()));
    }
}
