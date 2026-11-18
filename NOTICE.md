# Notice

The Xbox Series Dashboard is an unofficial fan project. It is not affiliated with, endorsed by or sponsored by Microsoft, Valve, Epic Games, GOG, Blizzard Entertainment, Electronic Arts, Ubisoft or Discord.

Xbox, Xbox Series X|S and the Xbox logo are trademarks of the Microsoft group of companies. Steam and the Steam logo are trademarks of Valve Corporation. Epic Games and its logo are trademarks of Epic Games, Inc. GOG and GOG.com and their logo are trademarks of GOG sp. z o.o. Battle.net and its logo are trademarks of Blizzard Entertainment, Inc. EA and its logo are trademarks of Electronic Arts Inc. Ubisoft and its logo are trademarks of Ubisoft Entertainment. Discord is a trademark of Discord Inc. They are used here only to describe what the Dashboard imitates and to name the stores your games come from.

## What is not the project's own

These are third-party material. They remain the property of their owners. They are included for personal, non-commercial use only, and no license this repository is offered under gives you any rights to them:

| Where | What it is | Owner | Taken from |
|---|---|---|---|
| `crates/screens/src/path_data.rs`, `marks::STEAM` | The Steam logo, as vector path data | Valve | The Xbox Series Theme's own path (`SourceSteam`), which is Simple Icons' `steam` |
| `crates/screens/src/path_data.rs`, `marks::XBOX` | The Xbox logo (the sphere), as vector path data | Microsoft | The Xbox Series Theme's own path (`SourceXbox`, `XboxLogo`), which is Simple Icons 9.21.0's `xbox`, the last release to carry it |
| `crates/screens/src/path_data.rs`, `marks::XBOX_GUIDE_PIECES`, `marks::XBOX_GUIDE_TOP` | The Xbox logo on the pad's Guide button, as vector path data | Microsoft | The Xbox Series Theme's idle screen, which takes it from Kenney's Input Prompts (`xbox_guide.svg`) |
| `crates/screens/src/path_data.rs`, `marks::EPIC_GAMES` | The Epic Games logo, as vector path data | Epic Games | Simple Icons 16.38.0, `epicgames` |
| `crates/screens/src/path_data.rs`, `marks::GOG` | The GOG.com logo, as vector path data | GOG | Simple Icons 16.38.0, `gogdotcom` |
| `crates/screens/src/path_data.rs`, `marks::BATTLE_NET` | The Battle.net logo, as vector path data | Blizzard Entertainment | Simple Icons 16.38.0, `battledotnet` |
| `crates/screens/src/path_data.rs`, `marks::EA` | The EA logo, as vector path data | Electronic Arts | Simple Icons 16.38.0, `ea` |
| `crates/screens/src/path_data.rs`, `marks::UBISOFT` | The Ubisoft logo, as vector path data | Ubisoft | Simple Icons 16.38.0, `ubisoft` |
| `crates/screens/tests/reference/*.png` | The screenshot tests' reference pictures, where they show the logos above | The logos' owners, as above | Drawn by the Dashboard |

The path data comes from [Simple Icons](https://simpleicons.org) (CC0 1.0) and [Kenney's Input Prompts](https://kenney.nl/assets/input-prompts) (CC0 1.0), through the [Xbox Series Theme](https://github.com/snrgarb/xbox-series-theme) where it has them. Their CC0 dedication covers their drawing of each path; the marks themselves remain their owners' trademarks.

The Dashboard uses the Segoe UI, Bahnschrift and Segoe Fluent Icons fonts that come with Windows. It does not include them. It includes none of the Xbox console's sounds or boot animations.

If you own any of this material and want it removed, open an issue at https://github.com/snrgarb/xbox-series-dashboard/issues.
