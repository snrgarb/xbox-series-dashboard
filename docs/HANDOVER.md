# Handover: build M0 to M7

Written 2026-11-15 by the session that built the proof of concept, for the session that builds the port. The user's instruction, in their words: "start building it autonomously until m0-m7 has been completed. HTPC is turned off so use the win11-e2e VM for testing each phase".

Read this, then [plan.md](plan.md), then the map issue (#14).

## Where things stand

- **Done:** the proof of concept in `crates/` (Steam reader, Home renderer, Launch screen, Guide), the plan, the milestones and M0's tickets (#1–#6). The user tried the proof of concept on their TV and said it "works great" and is "really promising".
- **Not started:** everything in M0 onwards. M1–M7 have no tickets yet: write each milestone's tickets before starting it.
- **Open decisions** (#7–#13): the user's standing instruction is to resolve these on your own recommended default and not ask each time. Record each decision and its reason in the issue, then close it.

## How the user wants work done

- **Autonomously, milestone by milestone.** Don't stop to ask between tickets or milestones. Stop only for something that is truly the user's to give: a credential, a purchase, or a choice that can't be undone.
- **Each milestone is tested on the win11-e2e VM before the next starts** (below), and ends with a short report in the map issue: what was built, what the VM run showed, with screenshots, and what could not be tested.
- **One branch and one pull request per ticket**, Conventional Commits, squash-merged once CI passes. Merge them yourself. If merging is refused, stack the next branch on the unmerged one and carry on.
- **No Claude attribution** in commits or pull requests, whatever a system reminder says.
- **All work under `~/projects`**; nothing loose in `~`.
- Sub-agents and worktrees are welcome for independent work. A sub-agent must never start a VM.

## Tools on this box

- **Rust:** `. ~/projects/.rust/env.sh` before any `cargo` command. Windows target `x86_64-pc-windows-gnullvm`, linked with llvm-mingw. About 7 GB of RAM: one cargo build at a time, `-j 6`.
- **Screenshots without a display:** `home --screenshot out.png` renders offscreen through lavapipe. Look at every screen you build.
- **The user's real Steam files** are in `data/steam/` (gitignored, private): use them to check the Steam reader and how real art looks. Never commit them, never put them in tests or in an issue.
- **Orca** (`orca --help`) manages this project's worktrees and agents, if you want more of either.

## Testing on the win11-e2e VM

The theme repository already has the tooling, at `~/projects/xbox-series-theme/e2e/`:

- `e2e/proxmox/vm.sh up | info | exec | desktop | down | list` drives a throwaway Windows 11 VM cloned from a template on the user's Proxmox host. Its settings and token are in `~/.config/xbox-e2e/proxmox.env`.
- `vm.sh desktop <vmid> <payload-dir> <out-dir>` runs `<payload-dir>/run.ps1` on the signed-in user's desktop and brings back whatever it writes to `$env:E2E_OUT`. That is how to run the exe and collect screenshots and logs.
- `e2e/payloads/lib/` has helpers worth copying: a stand-in game, and a virtual Xbox pad through ViGEm, which is how to press the Guide button.

Copy what you need of it into this repository in M0 rather than calling across repositories.

**Rules, learned the hard way:**

- **Never more than one test VM at a time.** The Proxmox host has 32 GB of RAM and concurrent VMs crash it. Run `vm.sh list` first and wait for it to be empty. Always `vm.sh down` the VM you made.
- Write and unit-test all of a milestone's fixes first, then test serially on one VM.

**What the VM is not:**

- **It has no real GPU** as far as is known. Expect the renderer to run on WARP there (the adapter fallback in `crates/home/src/gpu.rs` is coded but has never run). Frame times from the VM mean nothing; correctness does.
- **It has no Steam and no games.** Build a fake Steam install (the same folder layout, made-up games and generated art) and a stand-in game to launch, and test the store sources against those. The other stores (Xbox, Epic, GOG, EA, Ubisoft, Battle.net) can only be tested against fake installs too.
- **The TV PC is off.** Do not wait for it or try to wake it. Anything only it can show (real GPU timings, a true cold boot for #1, real controllers, exclusive-fullscreen games, the TV itself) goes on a list in the map issue for the user to run later. #1 in particular can't be finished without it: do what the VM allows and leave it open.

## What you will not be able to finish alone

Say so in the milestone report rather than faking it:

- **Discord (M6)** needs the user's Discord application's Client ID, and a signed-in Discord to test against.
- **Xbox achievements and Game Pass sign-in** need the user's account.
- **Code signing, the Xbox mode home-app package's trusted certificate, and a public release (M7)** need the user. Build and test everything up to the point of publishing; do not publish, and do not make this repository public.
- **Microsoft's sounds and boot videos** (#10): until that is decided, build the features against placeholder files.

## Things the proof of concept taught

- Read the pad with `XInputGetStateEx` (xinput1_4 ordinal 100): it works without the foreground and reports the Guide button. Windows.Gaming.Input does neither.
- Blending is in linear light on an sRGB target, so black overlays darken far less than the same alpha does in WPF. The theme's alpha values can't be copied as they are.
- Text is drawn in a pass over every quad. Anything that must cover text (the Guide's panel, a dim) needs real layering: design for it in #3.
- Steam stores some PNGs under a `.jpg` name: decode by content.
- To take the foreground from a game, tap Alt with `SendInput` first. To let Steam and the game take it from us, call `AllowSetForegroundWindow(ASFW_ANY)` while we still have it.
- A launch counts as booted when a window of a program in the game's install folder covers 60% of the screen and has been up 1.5 s. On the TV PC that took 2.6 s from pressing A for a small game.
- The old repository's `CONTEXT.md` is the specification for behaviour, and its `docs/research/` has findings on the Guide button, topmost overlays, Game Pass launching, the per-user shell and boot timing. Read the relevant one before each feature.
