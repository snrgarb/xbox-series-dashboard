## Project

Start with `docs/HANDOVER.md`. The Rust port of the Xbox Series Theme (`~/projects/xbox-series-theme`, the Playnite Bundle: its `CONTEXT.md` is the glossary and the behaviour to match, its `docs/research/` the prior findings). The plan is `docs/plan.md`; work is tracked in GitHub Issues on `snrgarb/xbox-series-dashboard`.

## Working here

- Toolchain: `. ~/projects/.rust/env.sh` before any `cargo` command. Windows target: `x86_64-pc-windows-gnullvm`. The box has ~7 GB RAM: one cargo build at a time.
- One build at a time means across worktrees too: run cargo as `flock ~/projects/.rust/build.lock cargo ... -j 6`, each worktree with its own `target/` (a shared target dir mixes worktrees' artifacts).
- How screens are built: `docs/ui.md`. What they should look like: `docs/spec/`. Tests that draw need `tools/test-fonts.sh` once. Screenshot tests are `#[ignore = "screenshot"]`, never in CI.
- Look at what you draw: `dashboard --screenshot <png>` (`cargo run -p app -- --screenshot <png>`) renders one frame offscreen (lavapipe here, DX12 on the TV PC over `ssh htpc-win`).
- `data/` holds a copy of the user's real Steam files for testing. It is private and gitignored: never commit it, and keep it out of tests.
- The TV PC (`ssh htpc-win`) is in daily use. Run only offscreen modes there unless asked to show something, and change nothing else on it.
- Fonts load from Windows at runtime, never bundled. Whether the console's sounds and boot videos come over is an open question in the plan: do not add them yet.
- Conventional Commits. No Claude attribution in commits or PRs.
