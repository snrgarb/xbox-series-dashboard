//! The page scripts (`js/*.js`): what they must never touch, and their behaviour in node against the made-up Discord of
//! `crates/standin/js/fake_discord.js` (`tests/js/run.js`).
use std::path::PathBuf;
use std::process::Command;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The page scripts never touch credentials and never call Discord's API themselves: they read stores that are
/// already in the page, and post what they read to the host. A build fails if one mentions a token, anything the
/// browser stores, a cookie, a password, an Authorization header or a network call.
#[test]
fn the_page_scripts_touch_no_credential_and_make_no_request() {
    let forbidden = [
        "token",
        "localstorage",
        "sessionstorage",
        "indexeddb",
        "cookie",
        "password",
        "authorization",
        "fetch(",
        "xmlhttprequest",
        "websocket",
        "eventsource",
        "sendbeacon",
        "importscripts",
        "eval(",
        "new function",
    ];
    for name in ["stores.js", "direct.js", "act.js", "voice.js"] {
        let text = std::fs::read_to_string(here().join("js").join(name)).unwrap().to_lowercase();
        for word in forbidden {
            assert!(!text.contains(word), "js/{name} mentions {word:?}");
        }
    }
    let script = discord_web::direct_script().to_lowercase();
    for word in forbidden {
        assert!(!script.contains(word), "the installed script mentions {word:?}");
    }
}

fn node() -> Option<String> {
    let out = Command::new("node").arg("--version").output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[test]
fn the_page_script_reads_pushes_and_opens_against_made_up_stores_in_node() {
    let Some(version) = node() else {
        eprintln!("SKIPPED: node is not installed; the page script's tests need it (CI's runner has it)");
        return;
    };
    let tmp = std::env::var_os("TMPDIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let dir = tempfile::tempdir_in(tmp).unwrap();
    let script = dir.path().join("script.js");
    std::fs::write(&script, discord_web::direct_script()).unwrap();
    let fake = here().join("../standin/js/fake_discord.js").canonicalize().unwrap();
    let out = Command::new("node").arg(here().join("tests/js/run.js")).arg(&script).arg(&fake).output().unwrap();
    let said = String::from_utf8_lossy(&out.stdout);
    eprintln!("node {version}:\n{said}{}", String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "a check failed:\n{said}");
    assert!(said.lines().filter(|l| l.starts_with("ok ")).count() >= 30, "too few checks ran:\n{said}");
}
