//! A stand-in for the update address on 127.0.0.1, for tests and `standin serve-updates`: no test asks GitHub. It
//! answers as GitHub's API does for a repository's newest release (`/releases/latest`, `tag_name` and `assets`),
//! and serves that release's portable zip, made here around the program it is given, and its `SHA256SUMS.txt`.
//! It keeps every request it was sent, so a test can say what was asked and what came with it.
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use crate::Version;

/// What the stand-in serves.
#[derive(Debug, Clone)]
pub struct Script {
    /// The newest release's version.
    pub version: Version,
    /// The program in its zip.
    pub exe: Vec<u8>,
    /// The manifest gives a SHA-256 that is not the zip's: the download must be refused.
    pub bad_hash: bool,
    /// `/releases/latest` answers 404, as GitHub's API does for a private repository.
    pub not_public: bool,
}

impl Script {
    pub fn new(version: Version, exe: Vec<u8>) -> Self {
        Self { version, exe, bad_hash: false, not_public: false }
    }
}

/// One request it was sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub path: String,
    pub user_agent: Option<String>,
    pub authorization: Option<String>,
}

pub struct FakeReleases {
    addr: SocketAddr,
    requests: Arc<Mutex<Vec<Request>>>,
}

struct Files {
    latest: String,
    zip_name: String,
    zip: Vec<u8>,
    sums: String,
    not_public: bool,
}

impl FakeReleases {
    /// Listens on 127.0.0.1 at `port` (0: any free one) and answers on a thread of its own for as long as the
    /// program runs.
    pub fn start(port: u16, script: Script) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        let addr = listener.local_addr()?;
        let zip_name = crate::zip_name(script.version);
        let zip = crate::zip::write_one(&format!("XboxSeriesDashboard-{}/{}", script.version, crate::EXE), &script.exe);
        // The bad manifest's hash is a real one, of other bytes: only the comparison can catch it.
        let hash = if script.bad_hash { crate::sha256(b"not the zip") } else { crate::sha256(&zip) };
        let base = format!("http://{addr}/download");
        let latest = serde_json::json!({
            "tag_name": format!("v{}", script.version),
            "name": format!("Xbox Series Dashboard {}", script.version),
            "assets": [
                {"name": zip_name, "browser_download_url": format!("{base}/{zip_name}")},
                {"name": crate::SUMS, "browser_download_url": format!("{base}/{}", crate::SUMS)},
            ],
        })
        .to_string();
        let sums = format!("{hash}  {zip_name}\n");
        let files = Arc::new(Files { latest, zip_name, zip, sums, not_public: script.not_public });
        let requests = Arc::new(Mutex::new(Vec::new()));
        let kept = requests.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (files, kept) = (files.clone(), kept.clone());
                std::thread::spawn(move || {
                    let _ = answer(stream, &files, &kept);
                });
            }
        });
        Ok(Self { addr, requests })
    }

    /// The update address, as the Dashboard is given it (`--update-source`).
    pub fn url(&self) -> String {
        format!("http://{}/releases/latest", self.addr)
    }

    /// Every request so far, in the order they came.
    pub fn requests(&self) -> Vec<Request> {
        self.requests.lock().unwrap().clone()
    }
}

fn answer(stream: TcpStream, files: &Files, kept: &Mutex<Vec<Request>>) -> std::io::Result<()> {
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
    let (status, kind, body): (&str, &str, &[u8]) = if path == "/releases/latest" {
        if files.not_public {
            ("404 Not Found", "application/json", br#"{"message":"Not Found","status":"404"}"#)
        } else {
            ("200 OK", "application/json", files.latest.as_bytes())
        }
    } else if path == format!("/download/{}", files.zip_name) {
        ("200 OK", "application/zip", &files.zip)
    } else if path == format!("/download/{}", crate::SUMS) {
        ("200 OK", "text/plain", files.sums.as_bytes())
    } else {
        ("404 Not Found", "text/plain", b"not here")
    };
    let mut stream = stream;
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}
