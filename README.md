# Xbox Series Dashboard

A dashboard for a Windows TV PC. It looks and works like the Xbox Series X|S dashboard. It is written in Rust and runs without Playnite. It is the port of the [Xbox Series Theme](https://github.com/snrgarb/xbox-series-theme) for Playnite.

This is an unofficial fan project. Microsoft, Valve, Discord and the game stores do not support or endorse it. Xbox and the Xbox logo are trademarks of Microsoft. For more information, see [NOTICE.md](NOTICE.md).

**Discord:** [join the server](https://discord.gg/9SqN9y2Sf3) to ask questions, report problems and see news.

## Features

| Feature | What it does |
|---|---|
| Home | Shows your recent games and apps. |
| The Guide | Opens over a game with the Guide button or Ctrl+Alt+G. |
| My games & apps | Shows all of your games from Steam, Xbox and Game Pass, Epic, GOG, EA, Ubisoft and Battle.net. |
| Game hub | Shows the art, achievements and actions for one game. |
| Search | Finds games and apps. |
| Installs | Installs, pauses and removes Steam, Epic and GOG games from the Queue. |
| Art | Gets game art from SteamGridDB with your own key. |
| Profiles | Asks "Who's playing today?" at start-up. Each person has a profile. |
| Idle screen | Dims the screen when you do not use it for some time. |
| Now Playing | Shows the game that is running. |
| Achievements | Shows Steam achievements, your score and rare achievements. |
| Discord | Shows your messages and calls. You can send messages, add reactions and join voice. |
| Shell mode | Replaces the Windows desktop on start-up. Set it in Settings > System. |
| Updates | The dashboard updates itself from the GitHub releases. |

The dashboard reads fonts from Windows. It does not include fonts, sounds or videos from the console.

## Install

There is no release yet. Until the first release, build the dashboard yourself (see [Build](#build)).

1. Go to [Releases](https://github.com/snrgarb/xbox-series-dashboard/releases).
2. Download the installer or the portable zip.
3. Start `dashboard.exe`. The dashboard finds your stores and opens in full screen.

To go back, push B or Esc. To close the dashboard, push Esc on Home.
The dashboard keeps its data and logs in `%LOCALAPPDATA%\XboxSeriesDashboard`.

## Build

You build the dashboard on Linux and cross-compile it for Windows.

You need:

- Rust (stable), from [rustup](https://rustup.rs).
- The Windows target: `rustup target add x86_64-pc-windows-gnullvm`.
- [llvm-mingw](https://github.com/mstorsjo/llvm-mingw/releases) as the linker. Put its `bin` folder on your `PATH`.

Then do these steps:

1. Set the linker:
   ```sh
   export CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_LINKER=x86_64-w64-mingw32-clang
   ```
2. Build `dashboard.exe` for Windows:
   ```sh
   cargo build --release --target x86_64-pc-windows-gnullvm
   ```
3. Get the fonts the tests draw with. Do this one time only:
   ```sh
   tools/test-fonts.sh
   ```
4. Run the tests on Linux:
   ```sh
   cargo test --workspace
   ```
   The screenshot tests, which compare a screen with a reference picture, are skipped. Run them by hand with
   `cargo nextest run -p screens --run-ignored only`.

To see one screen without a window, use `cargo run -p app -- --screenshot out.png`.

CI builds and tests each pull request. On a version tag, CI also makes the installer and the portable zip. For more information, see [docs/release.md](docs/release.md).

## Contributing

We welcome contributions. Do these steps:

1. Open an issue. Tell us what you want to change and why.
2. Make a branch from `main`.
3. Make your change. Keep one change in one pull request.
4. Use [Conventional Commits](https://www.conventionalcommits.org) for your commit messages, for example `fix(apps): ...`.
5. Make sure that these commands pass:
   ```sh
   cargo fmt --all --check
   cargo clippy --workspace --all-targets
   cargo test --workspace
   ```
6. Open a pull request. Link it to the issue.

Do not put personal data in code, tests or screenshots. This includes account names, IDs and game libraries.

For a screen, read [docs/ui.md](docs/ui.md) and [docs/spec/](docs/spec/) first. They tell you how a screen is built and how it must look.

To report a security problem, do not open an issue. Follow [SECURITY.md](SECURITY.md).

## License

You can use, change and share the dashboard for non-commercial purposes. You cannot sell it. For the full terms, see the [PolyForm Noncommercial License 1.0.0](LICENSE.md). The license does not cover third-party logos and trademarks. For more information, see [NOTICE.md](NOTICE.md).
