# Profiles specification

Who plays here: the Bundle's Profiles (its `CONTEXT.md`: "Profile", "Gamertag", "Signed-in Profile"), as the port keeps and shows them. #125. Who's playing today? and Switch profile, which choose among them, are #127's (start-up.md 5 to 8).

The Bundle's sources are cited as in settings.md: C/ for `XboxSeries.Common`, E/ for the extension, T/ for the theme. Settings' pages for them are settings.md 6.5, 6.6 and 4.8; where the port differs it says so here, in section 7.

## 1. What a Profile is

| Field | Bundle (C/StartUp.cs, `Profile`) | Port (`model::Profile`) |
|---|---|---|
| Id | a Guid | the database row's id, an integer |
| Gamertag | text, never empty | the same: trimmed, never empty |
| Real name | text, may be empty | the same: trimmed, may be empty |
| Picture | a file path, or empty for the Windows account's | `Picture::File` (a copy the database keeps) or `Picture::Shape(n)`, one of the port's plain pictures |

- **First name**: the real name's first word, or the gamertag if there is no real name (C/StartUp.cs `FirstName`). The greeting says "Hi {first name}!".
- **Has a real name**: a real name that is not blank. Home's top bar then swaps the gamertag and the real name (section 3).
- **Signed in**: one Profile at a time is the one playing. The port keeps which in the settings row `profile.last`; at start the one it names is signed in, or, if it is gone, the first (the Bundle's `LastProfileId`).

## 2. Where Profiles are kept

The library database (`library::Database`): a `profiles` table (migration 6) of id, gamertag, real name, picture (`shape:N` or a file name) and source (the picture file it was copied from, so the Picture page can tick it). Picture files are copied into `profiles/` in the data folder, named by the Profile's id and the time, and the old copy is deleted when another is worn; a Profile's copy is not touched by anything else, so a picture removed from where it came from still shows.

**The first Profile** is made the first time a database has none: its gamertag is the Windows user's name (`USERNAME`, else "Player"), its real name empty, its picture the account picture Windows records for the user (the registry's `AccountPicture\Users\<SID>\Image192`, copied in) if there is one, else plain picture 0. Nothing else is asked of Windows: no Microsoft account, no network, no NetUserGetInfo. The log says `profiles: made 1 "Name" from the Windows user, wearing its account picture` (or `a plain picture`).

A change is `Effect::Profile(ProfileChange)`: Add, Gamertag, RealName, Picture, Remove, SignIn. The executable carries it out on the database (`Store::change_profile`), logs it, and shows the snapshot after, so every place a Profile is drawn follows at once. What it refuses (a picture file that is not there; removing the last Profile; a Profile that is gone) changes nothing, and is said in a Toast ("There's no picture at ...").

**Removing the signed-in Profile signs in the first remaining** and remembers it. Removing the last is refused, here as in the Bundle.

## 3. Where a Profile shows

Everything draws `Dashboard::profile()`, the Profile signed in, through `screens::profile::picture` for its picture:

| Place | What |
|---|---|
| Home's top bar (home.md 2.1) | picture 56, gamertag; with a real name and Reduce motion off, the gamertag and the real name swap: gamertag for 7.2 s, a 0.3 s crossfade, the real name for 7.2 s, a 0.3 s crossfade back, a 15 s cycle on the ui's clock |
| The Guide's Profile & system tab icon (guide.md 3) | picture 54 |
| The Guide's Profile & system card, its profile entry (guide.md 4) | picture and gamertag |
| My games & apps' corner (my-games.md) | picture and gamertag |
| The greeting Toast | picture, "Hi {first name}!" |
| Settings > Profiles, a Profile's page, Picture | section 4 |

**Pictures.** A picture file is cropped to a circle (`UniformToFill`) over plain picture 0, which shows while it decodes and in place of one that will not load. The plain pictures are the port's own shapes: picture 0 is Windows' look for an account with no picture (a grey figure on a pale disc), which is what the Bundle's pictures show for its user; 1 to 7 are white motifs on coloured discs. No Microsoft, Xbox or anyone's art is in the repository.

A library with no database (a test's, `--library`) has no Profiles: the Profile shown is the store's user, or "Player", wearing plain picture 0, as before Profiles.

## 4. Settings > Personalisation > Profiles

Profiles is Personalisation's box at the Bundle's place, (1, 0), icon U+E716, beside Groups (settings.md 16.1). Its pages are the Bundle's (settings.md 6.5, 6.6, 9.3):

- **Profiles**: a box a Profile, two wide, then Add profile (U+E8FA). A Profile's box: its picture 80 at x + 30, its gamertag at x + 132. A opens the Profile's page; Add profile opens the Console keyboard for a gamertag ("Gamertag", empty), and one kept that is not blank is added last, focus on its box. Scrolls as settings.md 16.2.
- **A Profile** (header: its gamertag): its picture 120 at (520, 210), then Gamertag (U+E8EC), Real name (U+E8AC), Picture (U+E8B9), Remove profile (U+E74D) from y 360, two wide. Gamertag and Real name open the keyboard from what they are; a blank gamertag changes nothing, a blank real name clears it. Remove profile is not there while it is the only Profile; it asks "Remove {gamertag}?" with Remove and Cancel in the Dashboard's own prompt (the Bundle's is Playnite's message box), and Remove goes back to Profiles, focus on the box now in its place.
- **Picture**: gamerpic buttons (settings.md 4.8) four wide: the Windows account picture, then the `Gamerpics` folder's pictures in the data folder (made if missing; PNG, JPEG, GIF, BMP), sorted by name; then the eight plain pictures; then **From a file**, which opens the keyboard for a picture's full path and wears it. The one worn is ticked and has focus as the page opens. With at most one picture file to choose from the hint shows over them: "Add images to the Gamerpics folder to choose from more." A wears one and steps back.

Navigation is settings.md 9.3 on all three: Left and Right within the row, Up at the top and Left at column 0 do nothing, Down is min(here + columns, last). B steps back a page.

## 5. The focus log and the log

`--log-focus` lines (settings.md 16.6):

| Where | Line |
|---|---|
| Personalisation | `focus: Settings: Personalisation: Profiles` |
| Profiles | `focus: Settings: Profiles: <gamertag>`, `focus: Settings: Profiles: Add profile` |
| A Profile | `focus: Settings: Profile: <Gamertag / Real name / Picture / Remove profile>; "<gamertag>", real name "<real name>", <picture>` |
| Remove's question | `focus: Settings: Profile: Prompt(<Remove / Cancel>) of <gamertag>` |
| Picture | `focus: Settings: Picture: <file name / Shape N / From a file>`, and `; worn` on the one worn |
| Text entry | `focus: TextEntry: ...` as search.md |

`<picture>` is `shape N` or the picture file's name.

Always logged, with or without `--log-focus`:

- `profile: "<gamertag>", real name "<real name>", <picture>`: the Profile signed in, as everything shows it, at the first frame and whenever it changes.
- `profiles: <n> of them, <id> "<gamertag>" signed in last`, as the database is opened.
- `profiles: added <id> "<gamertag>"`, `profiles: <id> is now "<gamertag>"`, `profiles: <id>'s real name is "<real name>"`, `profiles: <id> wears shape N` / `wears <file>`, `profiles: removed <id> "<gamertag>"` (then `; <id> is signed in` if it was), `profiles: <id> "<gamertag>" signed in`; a refusal is `profiles: <why>`.

## 6. Choosing who plays

- `Dashboard::sign_in(id)` signs a Profile in: everything shows it at once, and it asks the database to remember it (`ProfileChange::SignIn`). Signing you in calls it (start-up.md 6): after Who's playing today? at the start, and from the Guide's Switch profile. Waking from sleep signs nobody in and asks nothing (start-up.md 8).
- Who's playing today? shows the Profiles in the database's order, focus on the one signed in last; its Personalisation box (0, 1) chooses whether it asks at the start (start-up.md 9).

## 7. Differences from the Bundle

- **Popular gamerpics are not fetched.** The Bundle fills the Gamerpics folder with the 100 most liked Xbox 360 gamerpics from xboxgamer.pics (E/PopularGamerpics.cs). The port offers its plain pictures and From a file instead (settings.md open question 8).
- **Plain pictures and From a file** are the port's: the Bundle has only picture files.
- **The picture is copied** into the data folder; the Bundle keeps the path, so a picture moved or deleted is lost.
- **Remove profile's question** is the Dashboard's prompt, not Playnite's message box, and its answers are Remove and Cancel, not Yes and No.
- **Removing the signed-in Profile** signs in the first; the Bundle keeps showing the removed one until the next start.
- **The first Profile's real name** is empty; the Bundle's is too, unless the user types one.
- **With no picture files**, the Picture box still opens its page (the plain pictures are there); the Bundle says "Add images to the Gamerpics folder to choose from." instead.
