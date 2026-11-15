# Xbox Series Dashboard

A standalone Windows dashboard for a TV PC, modelled on the Xbox Series X|S dashboard and written in Rust. It is the port of the [Xbox Series Theme](https://github.com/snrgarb/xbox-series-theme), which is a Playnite theme and extension, to its own executable with no Playnite underneath.

**Status: planning.** The code here is the proof of concept that started it: Home drawn with `wgpu` from the real Steam library, a game launched through Steam behind a Launch screen, and a Guide over Home and over a running game. See [docs/plan.md](docs/plan.md) for the port.

An unofficial fan project, not affiliated with Microsoft or Valve.

## Build

Rust with the `x86_64-pc-windows-gnullvm` target and llvm-mingw as its linker. On the development box: `. ~/projects/.rust/env.sh`.

```sh
cargo test                                                   # parsers, on Linux
cargo build --release --target x86_64-pc-windows-gnullvm     # home.exe for Windows
cargo run -p home -- --screenshot out.png --steam data/steam # one frame, no window (lavapipe on Linux)
```

`home.exe` with no arguments finds Steam from the registry and opens fullscreen. Guide button or Ctrl+Alt+G opens the Guide; Esc quits.
