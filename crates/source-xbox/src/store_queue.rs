//! The Store's install queue (#173; docs/research/installs.md 8, docs/research/game-pass.md 8): the one seam every
//! Xbox install, pause, resume, cancel and uninstall goes through, so that what is not known of it is in one small
//! place. [`Windows`] asks Windows' own `AppInstallManager` and `PackageManager` through `store_queue.ps1`, run by
//! Windows PowerShell; [`Fake`] stands in for it on the fake drive (`--xbox <dir>`), where nothing of the real
//! Store is ever asked. Each call says one line in the log, what was asked and what came back.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use model::{InstallState, Steer};
use serde::Deserialize;

/// The Store's install queue, as the Xbox installer asks it.
pub trait StoreQueue: Send + Sync {
    /// Asks the Store to install the product, for this user, onto the drive of `folder` where one is given (the
    /// Store's default otherwise). The state the Store first gave, as it names it, or why it refused.
    fn start(&self, product: &str, folder: Option<&Path>) -> Result<String, String>;

    /// Pauses, resumes or cancels the product's install.
    fn steer(&self, product: &str, steer: Steer) -> Result<(), String>;

    /// What the queue has now: every item, whoever asked for it.
    fn items(&self) -> Result<Vec<Item>, String>;

    /// Removes every package of the family for this user: how many.
    fn remove(&self, family: &str) -> Result<usize, String>;
}

/// An item of the queue, as `AppInstallItem` and its `AppInstallStatus` give it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Item {
    pub product: String,
    #[serde(default)]
    pub family: String,
    /// `AppInstallState`'s name: `Pending`, `Starting`, `AcquiringLicense`, `Downloading`, `RestoringData`,
    /// `Installing`, `Completed`, `Canceled`, `Paused`, `Error`, `PausedLowBattery`, `PausedWiFiRecommended`,
    /// `PausedWiFiRequired`, `ReadyToDownload`.
    pub state: String,
    #[serde(default)]
    pub percent: f64,
    #[serde(default)]
    pub done: u64,
    #[serde(default)]
    pub total: u64,
    /// The HRESULT of `ErrorCode`, 0 for none.
    #[serde(default)]
    pub error: i32,
}

/// Where an item is, as the Queue takes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Still under way: the Queue's state.
    Under(InstallState),
    Completed,
    Canceled,
    Failed,
}

impl Item {
    /// What the Queue shows of it. The bytes are the Store's own; where it gives a percent and no bytes (as while it
    /// acquires the licence), the percent is taken over 100 for the bar.
    pub fn stage(&self) -> Stage {
        let (done, total) = if self.total > 0 {
            (self.done.min(self.total), self.total)
        } else {
            ((self.percent.clamp(0.0, 100.0) * 100.0) as u64, if self.percent > 0.0 { 10_000 } else { 0 })
        };
        match self.state.as_str() {
            "Completed" => Stage::Completed,
            "Canceled" => Stage::Canceled,
            "Error" => Stage::Failed,
            "Pending" | "Starting" | "AcquiringLicense" | "ReadyToDownload" => Stage::Under(InstallState::Queued),
            "Downloading" | "RestoringData" | "Installing" => Stage::Under(InstallState::Downloading { done, total }),
            state if state.starts_with("Paused") => Stage::Under(InstallState::Paused { done, total }),
            _ => Stage::Under(InstallState::Queued),
        }
    }
}

/// Reads `watch`'s and `list`'s line: a JSON array of items. PowerShell 5.1 writes a lone item as an array too
/// (`ConvertTo-Json -InputObject @(...)`), and nothing as `[]`.
pub fn parse_items(line: &str) -> Result<Vec<Item>, String> {
    serde_json::from_str(line.trim()).map_err(|e| format!("the queue's answer is not its list ({e})"))
}

/// What a script's one-line answer says: `started <state>`, `ok`, `removed <n>`, or `refused 0x... <why>`.
pub fn parse_answer(line: &str) -> Result<String, String> {
    let line = line.trim();
    match line.strip_prefix("refused ") {
        Some(why) => Err(why.to_string()),
        None if line.is_empty() => Err("no answer".into()),
        None => Ok(line.to_string()),
    }
}

/// A product id the Store gives: twelve letters and digits. Nothing else is ever put in the script.
pub fn product_id(text: &str) -> Option<String> {
    (text.len() == 12 && text.bytes().all(|b| b.is_ascii_alphanumeric())).then(|| text.to_ascii_uppercase())
}

/// A package family name: `<name>_<publisher id>`, of letters, digits, `.`, `-` and `_`.
pub fn family_name(text: &str) -> Option<&str> {
    let ok = !text.is_empty()
        && text.len() <= 128
        && text.contains('_')
        && text.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b));
    ok.then_some(text)
}

/// The script with its arguments put first, as PowerShell's `-EncodedCommand` takes it: UTF-16LE in base64. Every
/// value is quoted as PowerShell's single-quoted strings are (a `'` doubled).
pub fn encoded(command: &str, values: &[(&str, &str)]) -> String {
    let quote = |text: &str| format!("'{}'", text.replace('\'', "''"));
    let mut script = format!("$Command = {}\n", quote(command));
    for (name, value) in values {
        script.push_str(&format!("${name} = {}\n", quote(value)));
    }
    script.push_str(SCRIPT);
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64(&bytes)
}

/// The script every call runs.
pub const SCRIPT: &str = include_str!("store_queue.ps1");

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(TABLE[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Windows' own queue, through Windows PowerShell 5.1 and `store_queue.ps1`. `items` keeps one `watch` running and
/// gives its last line: a look costs nothing once it runs.
#[derive(Default)]
pub struct Windows {
    watch: Mutex<Option<Watch>>,
}

struct Watch {
    last: Arc<Mutex<Option<String>>>,
    child: std::process::Child,
}

impl Windows {
    /// Runs the script for one answer, said in the log with how long it took.
    fn run(&self, command: &str, values: &[(&str, &str)]) -> Result<String, String> {
        let begun = Instant::now();
        let output = powershell(command, values)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .map_err(|e| format!("Windows PowerShell did not start ({e})"))?;
        let text = String::from_utf8_lossy(&output.stdout);
        let line = text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").to_string();
        let said = values.iter().map(|(_, v)| *v).filter(|v| !v.is_empty()).collect::<Vec<_>>().join(" ");
        log(&format!("store queue: {command} {said} -> {} ({} ms)", line.trim(), begun.elapsed().as_millis()));
        parse_answer(&line)
    }
}

fn powershell(command: &str, values: &[(&str, &str)]) -> std::process::Command {
    let mut run = std::process::Command::new("powershell.exe");
    run.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand"]);
    run.arg(encoded(command, values));
    run.stdin(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console comes up over the Dashboard.
        run.creation_flags(0x0800_0000);
    }
    run
}

impl StoreQueue for Windows {
    fn start(&self, product: &str, folder: Option<&Path>) -> Result<String, String> {
        let product = product_id(product).ok_or("not a product id")?;
        let volume = folder.map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        let started = self.run("start", &[("Product", &product), ("Volume", &volume)])?;
        Ok(started.strip_prefix("started ").unwrap_or(&started).to_string())
    }

    fn steer(&self, product: &str, steer: Steer) -> Result<(), String> {
        let product = product_id(product).ok_or("not a product id")?;
        self.run(steer.verb(), &[("Product", &product)]).map(|_| ())
    }

    fn items(&self) -> Result<Vec<Item>, String> {
        let mut watch = self.watch.lock().unwrap_or_else(|p| p.into_inner());
        let running = watch.as_mut().is_some_and(|w| matches!(w.child.try_wait(), Ok(None)));
        if !running {
            let parent = std::process::id().to_string();
            let mut child = powershell("watch", &[("Parent", &parent)])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|e| format!("Windows PowerShell did not start ({e})"))?;
            let out = child.stdout.take().ok_or("no output")?;
            let last = Arc::new(Mutex::new(None));
            let kept = last.clone();
            std::thread::spawn(move || {
                use std::io::BufRead;
                for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
                    if line.trim_start().starts_with('[') || line.trim_start().starts_with("refused") {
                        *kept.lock().unwrap_or_else(|p| p.into_inner()) = Some(line);
                    }
                }
            });
            log("store queue: watching the queue");
            *watch = Some(Watch { last, child });
        }
        let line = watch.as_ref().and_then(|w| w.last.lock().unwrap_or_else(|p| p.into_inner()).clone());
        match line {
            None => Ok(Vec::new()),
            Some(line) => match parse_answer(&line) {
                Err(why) => Err(why),
                Ok(_) => parse_items(&line),
            },
        }
    }

    fn remove(&self, family: &str) -> Result<usize, String> {
        let family = family_name(family).ok_or("not a package family name")?;
        let removed = self.run("remove", &[("Family", family)])?;
        removed.strip_prefix("removed ").and_then(|n| n.trim().parse().ok()).ok_or(removed)
    }
}

impl Drop for Windows {
    fn drop(&mut self) {
        if let Some(mut watch) = self.watch.lock().unwrap_or_else(|p| p.into_inner()).take() {
            let _ = watch.child.kill();
        }
    }
}

/// The stand-in for the Store's queue on the fake drive: each product of the fake catalogue
/// (`fixture::GAME_PASS`) "downloads" [`Fake::SIZE`] bytes at [`Fake::SPEED`] a second while not paused, and once
/// done is written onto the drive as a whole game (`fixture::write`), as the Store's install would register it;
/// `remove` deletes its folder. A product it has no game for is refused, and so is Ember Orchard ([`Fake::REFUSED`]),
/// as the real Store refuses one it will not license (`0x803F8001`), so the fallback to the Store's page is seen too. `DASHBOARD_XBOX_QUEUE_SECONDS` sets how long a whole
/// download takes (60 by default).
pub struct Fake {
    drive: PathBuf,
    seconds: f64,
    jobs: Mutex<HashMap<String, FakeJob>>,
}

#[derive(Debug, Clone)]
struct FakeJob {
    family: String,
    name: String,
    identity: String,
    /// The bytes done when last paused or started, and since when it has run (none while paused).
    done: f64,
    running_since: Option<Instant>,
    state: &'static str,
}

impl Fake {
    pub const SIZE: u64 = 2_000_000_000;
    /// The product it refuses: the fake catalogue's Ember Orchard.
    pub const REFUSED: &'static str = "9ZZZGPASS102";
    pub const SECONDS: &'static str = "DASHBOARD_XBOX_QUEUE_SECONDS";

    pub fn on(drive: &Path) -> Self {
        let seconds = std::env::var(Self::SECONDS).ok().and_then(|s| s.parse().ok()).filter(|&s: &f64| s > 0.0);
        Self::taking(drive, seconds.unwrap_or(60.0))
    }

    /// On `drive`, a whole download taking `seconds`.
    pub fn taking(drive: &Path, seconds: f64) -> Self {
        Self { drive: drive.to_path_buf(), seconds, jobs: Mutex::new(HashMap::new()) }
    }

    fn per_second(&self) -> f64 {
        Self::SIZE as f64 / self.seconds
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, FakeJob>> {
        self.jobs.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn done(&self, job: &FakeJob) -> f64 {
        let run = job.running_since.map_or(0.0, |since| since.elapsed().as_secs_f64() * self.per_second());
        (job.done + run).min(Self::SIZE as f64)
    }

    /// The fake catalogue's game of this product: its name and package identity.
    fn product(product: &str) -> Option<(&'static str, &'static str)> {
        crate::fixture::GAME_PASS
            .iter()
            .flat_map(|(_, games)| games.iter())
            .find(|(id, _, _)| id.eq_ignore_ascii_case(product))
            .map(|&(_, name, identity)| (name, identity))
    }
}

impl StoreQueue for Fake {
    fn start(&self, product: &str, _folder: Option<&Path>) -> Result<String, String> {
        let answer = match Self::product(product).filter(|_| !product.eq_ignore_ascii_case(Self::REFUSED)) {
            None => Err(format!("0x803F8001 the stand-in will not license {product}")),
            Some((name, identity)) => {
                let family = crate::family::family_name(identity, crate::fixture::PUBLISHER);
                let job = FakeJob {
                    family,
                    name: name.into(),
                    identity: identity.into(),
                    done: 0.0,
                    running_since: Some(Instant::now()),
                    state: "Downloading",
                };
                self.lock().insert(product.to_ascii_uppercase(), job);
                Ok("Pending".to_string())
            }
        };
        log(&format!("store queue (stand-in): start {product} -> {answer:?}"));
        answer
    }

    fn steer(&self, product: &str, steer: Steer) -> Result<(), String> {
        let mut jobs = self.lock();
        let answer = match jobs.get(&product.to_ascii_uppercase()).cloned() {
            None => Err(format!("0x80070490 no install of {product} in the queue")),
            Some(job) => {
                let done = self.done(&job);
                let job = jobs.get_mut(&product.to_ascii_uppercase()).expect("there");
                match steer {
                    Steer::Pause => {
                        job.done = done;
                        job.running_since = None;
                        job.state = "Paused";
                    }
                    Steer::Resume => {
                        if job.running_since.is_none() {
                            job.running_since = Some(Instant::now());
                        }
                        job.state = "Downloading";
                    }
                    Steer::Cancel => {
                        jobs.remove(&product.to_ascii_uppercase());
                    }
                }
                Ok(())
            }
        };
        log(&format!("store queue (stand-in): {} {product} -> {answer:?}", steer.verb()));
        answer
    }

    fn items(&self) -> Result<Vec<Item>, String> {
        let mut jobs = self.lock();
        let mut items = Vec::new();
        let mut finished = Vec::new();
        for (product, job) in jobs.iter() {
            let done = self.done(job);
            let complete = done >= Self::SIZE as f64;
            if complete {
                let spec = crate::fixture::Fixture {
                    games: vec![crate::fixture::FixtureGame {
                        name: job.name.clone(),
                        identity: job.identity.clone(),
                        kind: crate::fixture::Kind::Whole,
                    }],
                    game_exe: game_exe(&self.drive),
                };
                let written = crate::fixture::write(&self.drive, &spec).and_then(|()| {
                    // Its config's StoreId its own product's, as a real package's is.
                    let config = spec.games[0].package_root(&self.drive).join(crate::manifest::CONFIG);
                    let text = std::fs::read_to_string(&config)?;
                    std::fs::write(&config, text.replace("9ZZZMADEUP01", product))
                });
                let written = written.map_err(|e| e.to_string());
                log(&format!("store queue (stand-in): {product} complete, written onto the drive: {written:?}"));
                finished.push(product.clone());
            }
            items.push(Item {
                product: product.clone(),
                family: job.family.clone(),
                state: if complete { "Completed".into() } else { job.state.into() },
                percent: done * 100.0 / Self::SIZE as f64,
                done: done as u64,
                total: Self::SIZE,
                error: 0,
            });
        }
        for product in finished {
            jobs.remove(&product);
        }
        items.sort_by(|a, b| a.product.cmp(&b.product));
        Ok(items)
    }

    fn remove(&self, family: &str) -> Result<usize, String> {
        let mut removed = 0;
        let games = self.drive.join("XboxGames");
        for entry in std::fs::read_dir(&games).map_err(|e| e.to_string())?.flatten() {
            let content = entry.path().join("Content");
            let Ok(Some(game)) = crate::game_at(&content, None) else { continue };
            if game.source_id.eq_ignore_ascii_case(family) {
                std::fs::remove_dir_all(entry.path()).map_err(|e| e.to_string())?;
                removed += 1;
            }
        }
        log(&format!("store queue (stand-in): remove {family} -> removed {removed}"));
        Ok(removed)
    }
}

/// The program the fake drive's games carry (`standin make-xbox --game-exe`), for a game written onto it: the first
/// `XboxGames\<game>\Content\game.exe` there is.
fn game_exe(drive: &Path) -> Option<PathBuf> {
    let games = std::fs::read_dir(drive.join("XboxGames")).ok()?;
    games.flatten().map(|entry| entry.path().join("Content").join("game.exe")).find(|exe| exe.is_file())
}

fn log(line: &str) {
    eprintln!("xbox: {line}");
}

/// How often the installer looks at the queue while an install it asked for is under way.
pub const EVERY: Duration = Duration::from_secs(1);

#[cfg(test)]
mod tests {
    use super::*;
    use model::Source;

    #[test]
    fn the_queues_line_reads_as_its_items_and_their_stages() {
        let line = r#"[{"product":"9ZZZGPASS101","family":"MadeUp.Saltmarsh_0000000000000","state":"Downloading","percent":12.5,"done":250,"total":2000,"error":0},
            {"product":"9ZZZGPASS102","family":"","state":"AcquiringLicense","percent":0,"done":0,"total":0,"error":0},
            {"product":"9ZZZGPASS103","family":"","state":"PausedWiFiRequired","percent":40,"done":0,"total":0,"error":0},
            {"product":"9ZZZGPASS104","family":"","state":"Error","percent":3,"done":1,"total":9,"error":-2143330303}]"#;
        let items = parse_items(&line.replace('\n', "")).unwrap();
        let stages: Vec<Stage> = items.iter().map(Item::stage).collect();
        assert_eq!(
            stages,
            [
                Stage::Under(InstallState::Downloading { done: 250, total: 2000 }),
                Stage::Under(InstallState::Queued),
                Stage::Under(InstallState::Paused { done: 4000, total: 10_000 }),
                Stage::Failed,
            ]
        );
        assert_eq!(parse_items("[]").unwrap(), []);
        assert!(parse_items("refused 0x80070005 Access is denied.").is_err());
    }

    #[test]
    fn a_scripts_answer_is_its_line_or_why_it_refused() {
        assert_eq!(parse_answer("started Pending\r\n"), Ok("started Pending".into()));
        assert_eq!(parse_answer("refused 0x803F8001 no licence"), Err("0x803F8001 no licence".into()));
        assert_eq!(parse_answer("  "), Err("no answer".into()));
    }

    #[test]
    fn only_a_product_id_or_a_family_name_goes_into_the_script() {
        assert_eq!(product_id("9pd5bm2z8c4l").as_deref(), Some("9PD5BM2Z8C4L"));
        assert_eq!(product_id("9PD5BM2Z8C4'"), None);
        assert_eq!(product_id("9PD5"), None);
        assert_eq!(family_name("poncle.VampireSurvivors_9pv5cyp4vwdsr"), Some("poncle.VampireSurvivors_9pv5cyp4vwdsr"));
        assert_eq!(family_name("a'; Remove-Item C:\\ -Recurse; '_x"), None);
    }

    #[test]
    fn the_script_is_encoded_as_powershell_takes_it() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
        let encoded = encoded("start", &[("Product", "9ZZZGPASS101"), ("Volume", "D:\\It's")]);
        let bytes = decode(&encoded);
        let units: Vec<u16> = bytes.chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        let script = String::from_utf16(&units).unwrap();
        assert!(script.starts_with("$Command = 'start'\n$Product = '9ZZZGPASS101'\n$Volume = 'D:\\It''s'\n"));
        assert!(script.ends_with(SCRIPT));
    }

    fn decode(text: &str) -> Vec<u8> {
        const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = Vec::new();
        for chunk in text.as_bytes().chunks(4) {
            let mut n = 0u32;
            let mut pads = 0;
            for (i, &c) in chunk.iter().enumerate() {
                let v = if c == b'=' {
                    pads += 1;
                    0
                } else {
                    TABLE.iter().position(|&t| t == c).unwrap() as u32
                };
                n |= v << (18 - 6 * i);
            }
            for i in 0..3 - pads {
                out.push((n >> (16 - 8 * i)) as u8);
            }
        }
        out
    }

    #[test]
    fn the_stand_in_downloads_pauses_resumes_cancels_and_writes_the_game_when_done() {
        let dir = tempfile::tempdir().unwrap();
        crate::fixture::write(dir.path(), &crate::fixture::Fixture::standard()).unwrap();
        let queue = Fake { drive: dir.path().into(), seconds: 0.4, jobs: Mutex::new(HashMap::new()) };
        assert!(queue.start("9ZZZNOTHERE1", None).is_err(), "a product it has no game for is refused");
        assert_eq!(queue.start("9ZZZGPASS101", None), Ok("Pending".into()));
        queue.steer("9ZZZGPASS101", Steer::Pause).unwrap();
        let paused = queue.items().unwrap();
        assert!(matches!(paused[0].stage(), Stage::Under(InstallState::Paused { .. })), "{paused:?}");
        std::thread::sleep(Duration::from_millis(500));
        let still = queue.items().unwrap();
        assert_eq!(still[0].done, paused[0].done, "paused, it does not move");
        queue.steer("9ZZZGPASS101", Steer::Resume).unwrap();
        std::thread::sleep(Duration::from_millis(500));
        let done = queue.items().unwrap();
        assert_eq!(done[0].stage(), Stage::Completed);
        assert_eq!(queue.items().unwrap(), [], "once complete, it leaves the queue");
        let found = crate::Xbox::at(Some(dir.path().into())).read().unwrap().unwrap();
        let installed = found.library.games.iter().find(|g| g.name == "Saltmarsh Run").expect("written onto the drive");
        assert!(installed.installed);
        assert_eq!(queue.remove(&installed.source_id), Ok(1));
        let found = crate::Xbox::at(Some(dir.path().into())).read().unwrap().unwrap();
        assert!(found.library.games.iter().all(|g| g.name != "Saltmarsh Run"));

        assert!(queue.start(Fake::REFUSED, None).is_err(), "refused, as the Store refuses what it will not license");
        queue.start("9ZZZGPASS103", None).unwrap();
        queue.steer("9ZZZGPASS103", Steer::Cancel).unwrap();
        assert_eq!(queue.items().unwrap(), []);
        assert!(queue.steer("9ZZZGPASS103", Steer::Resume).is_err());
    }
}
