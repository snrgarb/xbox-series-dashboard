//! The commit the executable is built from, for Settings > About and `--version` (`DASHBOARD_COMMIT`): CI's
//! `DASHBOARD_COMMIT` where it gives one, else the checkout's own HEAD, else none (a build from a source archive).
use std::path::PathBuf;
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    let said = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (out.status.success() && !said.is_empty()).then_some(said)
}

fn main() {
    println!("cargo:rerun-if-env-changed=DASHBOARD_COMMIT");
    let given = std::env::var("DASHBOARD_COMMIT").ok().filter(|commit| !commit.trim().is_empty());
    let commit = given.map(|commit| commit.trim().chars().take(7).collect()).or_else(|| {
        // Built again when HEAD moves: a commit, a checkout, a rebase. Its reflog is the one file every one of
        // those writes, in a worktree too (whose git dir is its own).
        if let Some(dir) = git(&["rev-parse", "--absolute-git-dir"]) {
            for file in ["HEAD", "logs/HEAD"] {
                let path = PathBuf::from(&dir).join(file);
                if path.exists() {
                    println!("cargo:rerun-if-changed={}", path.display());
                }
            }
        }
        git(&["rev-parse", "--short=7", "HEAD"])
    });
    println!("cargo:rustc-env=DASHBOARD_COMMIT={}", commit.unwrap_or_default());
}
