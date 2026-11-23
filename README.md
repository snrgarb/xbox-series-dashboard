# Xbox Series Dashboard

A standalone Windows dashboard for a TV PC, modelled on the Xbox Series X|S dashboard and written in Rust. It is the port of the [Xbox Series Theme](https://github.com/snrgarb/xbox-series-theme), which is a Playnite theme and extension, to its own executable with no Playnite underneath.

**Status: M0 to M7 built, M8 (installs) under way.** Home, the Guide over games, Toasts, My games & apps, the Game hub, Search, Settings, Start-up, Profiles, the Idle screen, Now Playing, the Discord tab and achievements run from the libraries of Steam, Xbox, Epic, GOG, EA, Ubisoft and Battle.net, read from their own files. The Dashboard can be the PC's shell (Settings > System). Steam's, Epic's and GOG's games install, pause and uninstall from the Queue. See [docs/plan.md](docs/plan.md) for each milestone's state and [docs/HANDOVER.md](docs/HANDOVER.md) for how work is done.

An unofficial fan project, not affiliated with or endorsed by Microsoft, Valve, Discord or any of the stores it reads. Xbox and the Xbox logo are trademarks of Microsoft; the stores' logos it shows belong to their owners. See [NOTICE.md](NOTICE.md).

## Build and test

Rust with the `x86_64-pc-windows-gnullvm` target and llvm-mingw as its linker. On the development box: `. ~/projects/.rust/env.sh`. CI (`.github/workflows/ci.yml`) runs the tests on Linux, builds for Windows, and on a version tag packs an installer and a portable zip ([docs/release.md](docs/release.md)).

```sh
tools/test-fonts.sh                                        # once: the screenshot tests' fonts
cargo test -p screens --test shots                         # one crate's or one screen's tests, on Linux
cargo run -p app -- --screenshot out.png [--press ...]     # one frame, no window (lavapipe on Linux)
cargo build --release --target x86_64-pc-windows-gnullvm   # dashboard.exe for Windows (CI does this)
```

## Run

`dashboard.exe` with no arguments finds each store from the registry and opens fullscreen. The Guide button, or Ctrl+Alt+G, opens the Guide; Esc is B, and quits from Home with nothing open. Its data and logs are in `%LOCALAPPDATA%\XboxSeriesDashboard`. Fonts, and the console's sounds and boot videos, are never shipped: they are read from Windows and from a media folder at run time (`packaging/README.txt`).

End-to-end tests run on a Windows PC through `e2e/htpc/run.sh <test>` in a safe mode that changes nothing of the machine ([e2e/README.md](e2e/README.md)).

## License

The Xbox Series Dashboard is free for non-commercial use under the [PolyForm Noncommercial License 1.0.0](LICENSE.md). You may use, change and share it, but you may not sell it, and copies you share must keep the copyright notice. It is provided as is, without warranty. Third-party logos and marks are not covered by it: see [NOTICE.md](NOTICE.md).
