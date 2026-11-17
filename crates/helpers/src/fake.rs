//! A stand-in for the helpers' release pages on 127.0.0.1, for tests and `standin serve-helpers`: no test asks
//! GitHub. `/helpers/good` is the program it is given; `/helpers/bad` is other bytes, which a table expecting the
//! good one's SHA-256 must refuse. It keeps every request it was sent, so a test can say what was asked and what
//! came with it.
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use crate::Helper;

/// One request it was sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub path: String,
    pub user_agent: Option<String>,
    pub authorization: Option<String>,
}

pub struct FakeHelpers {
    addr: SocketAddr,
    good: Arc<Vec<u8>>,
    requests: Arc<Mutex<Vec<Request>>>,
}

/// What `/helpers/bad` serves: not the program, whatever it is.
fn bad(good: &[u8]) -> Vec<u8> {
    let mut bytes = b"not the helper ".to_vec();
    bytes.extend_from_slice(good);
    bytes
}

impl FakeHelpers {
    /// Listens on 127.0.0.1 at `port` (0: any free one) and answers on a thread of its own for as long as the
    /// program runs.
    pub fn start(port: u16, good: Vec<u8>) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        let addr = listener.local_addr()?;
        let good = Arc::new(good);
        let requests = Arc::new(Mutex::new(Vec::new()));
        let (served, kept) = (good.clone(), requests.clone());
        std::thread::spawn(move || {
            let bad = Arc::new(bad(&served));
            for stream in listener.incoming().flatten() {
                let (good, bad, kept) = (served.clone(), bad.clone(), kept.clone());
                std::thread::spawn(move || {
                    let _ = answer(stream, &good, &bad, &kept);
                });
            }
        });
        Ok(Self { addr, good, requests })
    }

    /// A file's address: `good` or `bad`.
    pub fn url(&self, name: &str) -> String {
        format!("http://{}/helpers/{name}", self.addr)
    }

    /// The builtin table with its addresses here, each expecting the good program's SHA-256: legendary is served
    /// the good one, gogdl the bad, which must be refused.
    pub fn table(&self) -> Vec<Helper> {
        let hash = crate::sha256(&self.good);
        crate::builtin()
            .into_iter()
            .map(|helper| {
                let file = if helper.key == "legendary" { "good" } else { "bad" };
                Helper {
                    url: self.url(file),
                    sha256: Some(hash.clone()),
                    sha256_from: "the stand-in's own program".into(),
                    ..helper
                }
            })
            .collect()
    }

    /// The launchers' apps' table (`crate::clients`) with their addresses here, for a test's machine: every
    /// installer is this server's good program, run as itself (`standin-setup-<key>.exe`, the stand-in installer,
    /// which writes its app's Uninstall key in the test's own registry file), with each app's own switches; an MSI
    /// is run as a program. GOG Galaxy's address serves the bad bytes, which must be refused. An installer told where
    /// to go is told `<sandbox>\<key>`.
    pub fn client_table(&self, sandbox: &std::path::Path) -> Vec<crate::clients::Client> {
        let hash = crate::sha256(&self.good);
        crate::clients::builtin()
            .into_iter()
            .map(|client| {
                let file = if client.key == "gog" { "bad" } else { "good" };
                crate::clients::Client {
                    url: self.url(file),
                    size: self.good.len() as u64,
                    sha256: Some(hash.clone()),
                    sha256_from: "the stand-in's own program".into(),
                    file: format!("standin-setup-{}.exe", client.key),
                    kind: match client.kind {
                        crate::clients::Kind::Msi => crate::clients::Kind::Exe,
                        kind => kind,
                    },
                    location: client.location.as_ref().map(|_| sandbox.join(&client.key).display().to_string()),
                    ..client
                }
            })
            .collect()
    }

    /// Every request so far, in the order they came.
    pub fn requests(&self) -> Vec<Request> {
        self.requests.lock().unwrap().clone()
    }
}

fn answer(stream: TcpStream, good: &[u8], bad: &[u8], kept: &Mutex<Vec<Request>>) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
    let mut request = Request { path: path.clone(), user_agent: None, authorization: None };
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            let value = Some(value.trim().to_string());
            match name.trim().to_ascii_lowercase().as_str() {
                "user-agent" => request.user_agent = value,
                "authorization" => request.authorization = value,
                _ => {}
            }
        }
    }
    kept.lock().unwrap().push(request);
    let (status, body): (&str, &[u8]) = match path.as_str() {
        "/helpers/good" => ("200 OK", good),
        "/helpers/bad" => ("200 OK", bad),
        _ => ("404 Not Found", b"not here"),
    };
    let mut stream = stream;
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}
