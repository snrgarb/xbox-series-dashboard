//! The Guide's Discord tab (docs/spec/discord-tab.md): what it lists as Discord says, where focus goes as that
//! changes, Mute, Deafen and Leave asked of Discord and shown done only once Discord says so, and what each state
//! looks like. `SHOTS=bless cargo test -p screens --test discord`.
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use model::discord::{Ask, Browse, BrowseAsk, Call, CallAsk, Discord, Person, Server, Shortcut, Voice, VoiceChannel};
use render::gpu::{self, Gpu};
use render::offscreen::FORMAT;
use screens::guide::{DiscordItem as Item, Flyout, FlyoutFocus, Focus, Kind, Tab};
use screens::{Dashboard, Effect};
use ui::host::Offscreen;
use ui::Button::{self, Down, Guide, Right, RightBumper, Up, A, B};
use ui::{Host, Press};

fn press(dashboard: &mut Dashboard, buttons: &[Button]) {
    for &button in buttons {
        dashboard.press(Press { button, repeat: false });
    }
}

/// What has been asked of the shell, but for the sounds.
fn asked(dashboard: &mut Dashboard) -> Vec<Effect> {
    dashboard.take_effects().into_iter().filter(|effect| !matches!(effect, Effect::Sound(_))).collect()
}

fn items(dashboard: &Dashboard) -> Vec<Item> {
    let item = |kind: Kind| match kind {
        Kind::Discord(item) => Some(item),
        _ => None,
    };
    dashboard.guide().entries().iter().filter_map(|entry| item(entry.kind)).collect()
}

fn focus(dashboard: &Dashboard) -> (Tab, Focus) {
    dashboard.guide().focus()
}

fn person(id: &str, name: &str) -> Person {
    Person { id: id.into(), name: name.into(), avatar: None, speaking: false, muted: false, deafened: false, me: false }
}

/// Signed in as E2E Tester, out of a Call.
fn signed_in() -> Discord {
    Discord { connected: true, user: Some("E2E Tester".into()), ..Discord::default() }
}

/// In General with these people, the user first.
fn in_call(others: &[(&str, &str)]) -> Discord {
    let me = Person { me: true, ..person("1", "E2E Tester") };
    let people = std::iter::once(me).chain(others.iter().map(|&(id, name)| person(id, name))).collect();
    let call = Call { id: "500".into(), title: "General".into(), people };
    Discord { voice: Voice { call: Some(call), mute: false, deaf: false }, ..signed_in() }
}

fn people(discord: &mut Discord) -> &mut Vec<Person> {
    &mut discord.voice.call.as_mut().unwrap().people
}

/// Home with the Guide open on the Discord tab, Discord as given.
fn on_tab(discord: &Discord) -> Dashboard {
    let mut dashboard = Dashboard::new(screens::fake_library());
    dashboard.set_discord(discord);
    press(&mut dashboard, &[Guide, RightBumper]);
    dashboard.take_effects();
    assert_eq!(focus(&dashboard).0, Tab::Discord);
    dashboard
}

#[test]
fn out_of_a_call_it_shows_who_is_signed_in_then_servers_which_has_the_first_focus() {
    let mut dashboard = on_tab(&signed_in());
    assert_eq!(items(&dashboard), [Item::Account, Item::Idle, Item::Servers]);
    assert_eq!(dashboard.guide().entries()[1].title, "You're not in a voice channel");
    assert_eq!(focus(&dashboard), (Tab::Discord, Focus::Entry(2)));
    assert_eq!(dashboard.guide_line(), "Discord Entry(2) Servers; not in a call; mute off, deafen off");
    // A on who is signed in, or on the line under it, does nothing.
    press(&mut dashboard, &[Up, Up, A, Down, A]);
    assert!(asked(&mut dashboard).is_empty() && dashboard.guide_open());
    assert_eq!(dashboard.guide_line(), "Discord Entry(1) Idle; not in a call; mute off, deafen off");
}

#[test]
fn in_a_call_it_lists_the_call_then_mute_deafen_and_leave_with_focus_on_mute() {
    let mut dashboard = on_tab(&in_call(&[("2", "Ana"), ("3", "Bo")]));
    let call = [
        Item::Channel,
        Item::Person(0),
        Item::Person(1),
        Item::Person(2),
        Item::Mute,
        Item::Deafen,
        Item::Leave,
        Item::Servers,
    ];
    assert_eq!(items(&dashboard), call);
    assert_eq!(focus(&dashboard), (Tab::Discord, Focus::Entry(4)));
    let titles: Vec<&str> = dashboard.guide().entries().iter().map(|entry| entry.title.as_str()).collect();
    assert_eq!(titles, ["General", "E2E Tester", "Ana", "Bo", "Mute", "Deafen", "Leave", "Servers"]);
    // A on the Call's heading or someone in it does nothing.
    press(&mut dashboard, &[Up, A, Up, Up, Up, A]);
    assert_eq!(focus(&dashboard), (Tab::Discord, Focus::Entry(0)));
    assert!(asked(&mut dashboard).is_empty());
    // Servers is last: down from it stays.
    press(&mut dashboard, &[Down; 9]);
    assert_eq!(focus(&dashboard), (Tab::Discord, Focus::Entry(7)));
}

#[test]
fn mute_and_deafen_are_asked_of_discord_and_switch_only_once_discord_says_so() {
    let call = in_call(&[("2", "Ana")]);
    let mut dashboard = on_tab(&call);
    press(&mut dashboard, &[A]);
    assert_eq!(asked(&mut dashboard), [Effect::Discord(Ask::Call(CallAsk::Mute))]);
    assert!(dashboard.guide_line().ends_with("mute off, deafen off"), "{}", dashboard.guide_line());

    let mut muted = call.clone();
    muted.voice.mute = true;
    people(&mut muted)[0].muted = true;
    dashboard.set_discord(&muted);
    assert_eq!(
        dashboard.guide_line(),
        "Discord Entry(3) Mute; in \"General\" with E2E Tester (muted), Ana; mute on, deafen off"
    );

    press(&mut dashboard, &[Down, A]);
    assert_eq!(asked(&mut dashboard), [Effect::Discord(Ask::Call(CallAsk::Deafen))]);
    assert!(dashboard.guide_line().ends_with("mute on, deafen off"));
}

#[test]
fn leave_is_asked_of_discord_and_the_call_goes_only_once_discord_says_so() {
    let call = in_call(&[("2", "Ana")]);
    let mut dashboard = on_tab(&call);
    press(&mut dashboard, &[Down, Down, A]);
    assert_eq!(asked(&mut dashboard), [Effect::Discord(Ask::Call(CallAsk::Leave))]);
    assert_eq!(focus(&dashboard), (Tab::Discord, Focus::Entry(5)), "still on Leave");
    assert!(dashboard.guide_open(), "the Guide stays open");

    // Out of the Call: focus starts afresh, on the Shortcut to the channel just left.
    let left = Discord { shortcuts: vec![shortcut("500", "General", Some("Stand-in Server"))], ..signed_in() };
    dashboard.set_discord(&left);
    assert_eq!(items(&dashboard), [Item::Account, Item::Idle, Item::Shortcut(0), Item::Servers]);
    assert_eq!(focus(&dashboard), (Tab::Discord, Focus::Entry(2)));
    assert_eq!(
        dashboard.guide_line(),
        "Discord Entry(2) Shortcut \"General\" (Stand-in Server); not in a call; mute off, deafen off"
    );
    // Into one again: on Mute.
    dashboard.set_discord(&call);
    assert_eq!(focus(&dashboard), (Tab::Discord, Focus::Entry(3)));
}

#[test]
fn focus_stays_with_its_person_or_entry_as_people_come_and_go() {
    let three = in_call(&[("2", "Ana"), ("3", "Bo")]);
    let mut dashboard = on_tab(&three);
    // On Bo; Ana leaves: still on Bo.
    press(&mut dashboard, &[Up]);
    assert!(dashboard.guide_line().starts_with("Discord Entry(3) Person Bo;"));
    dashboard.set_discord(&in_call(&[("3", "Bo")]));
    assert!(dashboard.guide_line().starts_with("Discord Entry(2) Person Bo;"), "{}", dashboard.guide_line());
    // On Deafen; two join: still on Deafen.
    press(&mut dashboard, &[Down, Down]);
    dashboard.set_discord(&in_call(&[("3", "Bo"), ("4", "Cy"), ("5", "Di")]));
    assert!(dashboard.guide_line().starts_with("Discord Entry(6) Deafen;"), "{}", dashboard.guide_line());
    // Someone speaking or muting changes nothing of focus.
    let mut speaking = in_call(&[("3", "Bo"), ("4", "Cy"), ("5", "Di")]);
    people(&mut speaking)[2].speaking = true;
    dashboard.set_discord(&speaking);
    assert!(dashboard
        .guide_line()
        .starts_with("Discord Entry(6) Deafen; in \"General\" with E2E Tester, Bo, Cy (speaking)"));
}

#[test]
fn the_voice_going_mid_call_takes_the_tab_away() {
    let mut dashboard = on_tab(&in_call(&[("2", "Ana")]));
    let gone = Discord { connected: false, user: None, voice: Voice::default(), ..signed_in() };
    dashboard.set_discord(&gone);
    assert_eq!(dashboard.guide().tabs(), [Tab::Landing, Tab::ProfileAndSystem]);
    assert_eq!(focus(&dashboard).0, Tab::Landing);
}

#[test]
fn the_tab_is_there_while_the_web_sessions_voice_is_ready() {
    let mut dashboard = Dashboard::new(screens::fake_library());
    // Not ready: none.
    dashboard.set_discord(&Discord { connected: false, user: None, ..signed_in() });
    press(&mut dashboard, &[Guide]);
    assert_eq!(dashboard.guide().tabs(), [Tab::Landing, Tab::ProfileAndSystem]);
    press(&mut dashboard, &[B]);
    // Ready: it stands between the two.
    dashboard.set_discord(&signed_in());
    press(&mut dashboard, &[Guide]);
    assert_eq!(dashboard.guide().tabs(), [Tab::Landing, Tab::Discord, Tab::ProfileAndSystem]);
}

fn shortcut(id: &str, name: &str, server: Option<&str>) -> Shortcut {
    let server = server.map(str::to_string);
    Shortcut { id: id.into(), name: name.into(), dm: server.is_none(), server, picture: None }
}

/// Signed in with five Shortcuts: two rows, of three and two.
fn five_shortcuts() -> Discord {
    let shortcuts = vec![
        shortcut("500", "General", Some("Stand-in Server")),
        shortcut("600", "Ana", None),
        shortcut("510", "Lobby", Some("Another Server")),
        shortcut("601", "Ana, Bo", None),
        shortcut("501", "Late night", Some("Stand-in Server")),
    ];
    Discord { shortcuts, ..signed_in() }
}

#[test]
fn the_shortcuts_stand_three_to_a_row_before_servers_and_a_rejoins_one() {
    let mut dashboard = on_tab(&five_shortcuts());
    let tiles: Vec<Item> = (0..5).map(Item::Shortcut).collect();
    assert_eq!(items(&dashboard)[2..7], tiles[..]);
    assert_eq!(focus(&dashboard), (Tab::Discord, Focus::Entry(2)), "the first Shortcut");
    // Along the first row, held at its end; down onto the shorter row's last; on to Servers; back up.
    press(&mut dashboard, &[Right, Right, Right]);
    assert_eq!(focus(&dashboard).1, Focus::Entry(4));
    press(&mut dashboard, &[Down]);
    assert_eq!(focus(&dashboard).1, Focus::Entry(6));
    press(&mut dashboard, &[Down]);
    assert_eq!(focus(&dashboard).1, Focus::Entry(7), "Servers");
    press(&mut dashboard, &[Up]);
    assert_eq!(focus(&dashboard).1, Focus::Entry(5), "the last row's first");
    press(&mut dashboard, &[Up, Up]);
    assert_eq!(focus(&dashboard).1, Focus::Entry(1), "the entry above the tiles");
    press(&mut dashboard, &[Down, Right, A]);
    assert_eq!(asked(&mut dashboard), [Effect::Discord(Ask::Join("600".into()))]);
    assert!(dashboard.guide_line().starts_with("Discord Entry(3) Shortcut \"Ana\"; not in a call"));
}

#[test]
fn a_shortcuts_menu_removes_it_and_focus_keeps_its_place() {
    let mut dashboard = on_tab(&five_shortcuts());
    press(&mut dashboard, &[Right, Button::Menu]);
    assert_eq!(dashboard.guide().dialog_title(), Some("Ana"));
    // Discord answering something else while the menu is up leaves it up: no game is behind it.
    let mut speaking = five_shortcuts();
    speaking.voice.mute = true;
    dashboard.set_discord(&speaking);
    assert_eq!(dashboard.guide().dialog_title(), Some("Ana"));
    press(&mut dashboard, &[A]);
    assert_eq!(asked(&mut dashboard), [Effect::Discord(Ask::RemoveShortcut("600".into()))]);
    assert_eq!(dashboard.guide().dialog_title(), None);

    let mut removed = five_shortcuts();
    removed.shortcuts.remove(1);
    dashboard.set_discord(&removed);
    assert!(dashboard.guide_line().starts_with("Discord Entry(3) Shortcut \"Lobby\""), "{}", dashboard.guide_line());
}

#[test]
fn a_shortcuts_menu_goes_with_its_shortcut() {
    let mut dashboard = on_tab(&five_shortcuts());
    press(&mut dashboard, &[Button::Menu]);
    let mut removed = five_shortcuts();
    removed.shortcuts.remove(0);
    dashboard.set_discord(&removed);
    assert_eq!(dashboard.guide().dialog_title(), None);
}

fn servers(names: &[(&str, &str)]) -> Browse {
    let servers = names.iter().map(|&(id, name)| Server { id: id.into(), name: name.into(), icon: None }).collect();
    Browse::Servers { loading: false, servers }
}

fn channels(of: (&str, &str), channels: &[(&str, &str, Option<usize>)]) -> Browse {
    let channels = channels
        .iter()
        .map(|&(id, name, count)| VoiceChannel {
            id: id.into(),
            name: name.into(),
            occupants: count.map(|count| vec![None; count]),
        })
        .collect();
    Browse::Server { id: of.0.into(), name: of.1.into(), loading: false, channels }
}

fn browsing(browse: Browse) -> Discord {
    Discord { browse, ..signed_in() }
}

const THREE: [(&str, &str); 3] = [("902", "Another Server"), ("903", "art club"), ("901", "Stand-in Server")];

#[test]
fn servers_opens_its_flyout_at_once_loading_and_a_server_opens_its_voice_channels() {
    let mut dashboard = on_tab(&signed_in());
    press(&mut dashboard, &[A]);
    assert_eq!(asked(&mut dashboard), [Effect::Discord(Ask::Browse(BrowseAsk::Servers))]);
    assert_eq!(dashboard.flyout(), Some((Flyout::Discord, FlyoutFocus::Nothing)));
    // Nothing Discord says yet closes it.
    dashboard.set_discord(&signed_in());
    dashboard.set_discord(&browsing(Browse::Servers { loading: true, servers: vec![] }));
    assert!(
        dashboard.guide_line().ends_with(", flyout Discord Nothing \"Servers\" loading"),
        "{}",
        dashboard.guide_line()
    );

    dashboard.set_discord(&browsing(servers(&THREE)));
    assert_eq!(dashboard.flyout(), Some((Flyout::Discord, FlyoutFocus::Item(0))));
    press(&mut dashboard, &[Down, Down, Down, A]);
    let lads = BrowseAsk::Server { id: "901".into(), name: "Stand-in Server".into() };
    assert_eq!(asked(&mut dashboard), [Effect::Discord(Ask::Browse(lads))]);

    let general = ("500", "General", Some(0));
    dashboard.set_discord(&browsing(channels(("901", "Stand-in Server"), &[general, ("501", "Late night", Some(2))])));
    assert!(
        dashboard.guide_line().ends_with(", flyout Discord Item(0) \"Stand-in Server\" \"General\" of 2: Empty"),
        "{}",
        dashboard.guide_line()
    );
    press(&mut dashboard, &[Down]);
    assert!(dashboard.guide_line().ends_with("\"Late night\" of 2: 2 in it"), "{}", dashboard.guide_line());

    // A joins it: the Flyout closes at once, focus on Servers.
    press(&mut dashboard, &[A]);
    assert_eq!(asked(&mut dashboard), [Effect::Discord(Ask::Join("501".into()))]);
    assert_eq!(dashboard.flyout(), None);
    assert!(dashboard.guide_line().starts_with("Discord Entry(2) Servers;"));
}

#[test]
fn b_goes_back_to_the_servers_onto_the_one_it_came_from_then_closes() {
    let mut dashboard = on_tab(&signed_in());
    press(&mut dashboard, &[A]);
    dashboard.set_discord(&browsing(servers(&THREE)));
    press(&mut dashboard, &[Down, A]);
    dashboard.set_discord(&browsing(channels(("903", "art club"), &[])));
    assert!(
        dashboard.guide_line().ends_with("Nothing \"art club\" \"No voice channels\""),
        "{}",
        dashboard.guide_line()
    );
    asked(&mut dashboard);

    press(&mut dashboard, &[B]);
    assert_eq!(asked(&mut dashboard), [Effect::Discord(Ask::Browse(BrowseAsk::Back))]);
    dashboard.set_discord(&browsing(servers(&THREE)));
    assert_eq!(dashboard.flyout(), Some((Flyout::Discord, FlyoutFocus::Item(1))), "on art club");

    press(&mut dashboard, &[B]);
    assert_eq!(asked(&mut dashboard), [Effect::Discord(Ask::Browse(BrowseAsk::Close))]);
    assert_eq!(dashboard.flyout(), None);
    assert!(dashboard.guide_open());
}

/// As on the PC: A on a server is carried out after the press, its channels a frame or more later, the occupants
/// after that; B's servers come back as it is carried out. Focus stays on the row pressed until Discord shows where
/// A or B went, and the line says where it really is all along.
#[test]
fn focus_stays_on_the_server_pressed_until_discord_shows_its_channels_and_b_comes_back_onto_it() {
    let lads = ("901", "Stand-in Server");
    let line = |dashboard: &Dashboard| dashboard.guide_line();
    let on = |what: &str| format!(", flyout Discord {what}");
    let mut dashboard = on_tab(&signed_in());
    press(&mut dashboard, &[A]);
    dashboard.set_discord(&browsing(Browse::Servers { loading: true, servers: vec![] }));
    dashboard.set_discord(&browsing(servers(&THREE)));
    press(&mut dashboard, &[Down, Down]);
    let on_lads = on("Item(2) \"Servers\" \"Stand-in Server\" of 3");
    assert!(line(&dashboard).ends_with(&on_lads), "{}", line(&dashboard));
    asked(&mut dashboard);

    // A: asked, and until Discord answers focus is still on Stand-in Server.
    press(&mut dashboard, &[A]);
    assert_eq!(asked(&mut dashboard).len(), 1);
    assert!(line(&dashboard).ends_with(&on_lads), "after A, before Discord: {}", line(&dashboard));
    assert_eq!(dashboard.flyout(), Some((Flyout::Discord, FlyoutFocus::Item(2))));
    dashboard.set_discord(&browsing(Browse::Server {
        id: lads.0.into(),
        name: lads.1.into(),
        loading: true,
        channels: vec![],
    }));
    assert!(line(&dashboard).ends_with(&on("Nothing \"Stand-in Server\" loading")), "{}", line(&dashboard));
    dashboard.set_discord(&browsing(channels(lads, &[("500", "General", None), ("501", "Late night", None)])));
    assert!(line(&dashboard).ends_with("Item(0) \"Stand-in Server\" \"General\" of 2: not known yet"));
    dashboard.set_discord(&browsing(channels(lads, &[("500", "General", Some(0)), ("501", "Late night", Some(2))])));
    assert!(line(&dashboard).ends_with("Item(0) \"Stand-in Server\" \"General\" of 2: Empty"));

    // B from Late night: the channels still show until Discord shows the servers, then focus is on Stand-in Server.
    press(&mut dashboard, &[Down, B]);
    assert_eq!(asked(&mut dashboard), [Effect::Discord(Ask::Browse(BrowseAsk::Back))]);
    assert!(line(&dashboard).ends_with("Item(1) \"Stand-in Server\" \"Late night\" of 2: 2 in it"));
    dashboard.set_discord(&browsing(servers(&THREE)));
    assert!(line(&dashboard).ends_with(&on_lads), "after B: {}", line(&dashboard));

    // A again opens Stand-in Server, the one the line says; its channels known, they show as it is carried out.
    press(&mut dashboard, &[A]);
    let ask = BrowseAsk::Server { id: lads.0.into(), name: lads.1.into() };
    assert_eq!(asked(&mut dashboard), [Effect::Discord(Ask::Browse(ask))]);
    dashboard.set_discord(&browsing(channels(lads, &[("500", "General", Some(0)), ("501", "Late night", Some(2))])));
    press(&mut dashboard, &[Down]);
    assert!(line(&dashboard).ends_with("Item(1) \"Stand-in Server\" \"Late night\" of 2: 2 in it"));
}

/// As on the PC: the menu closes on A, and Discord confirms the removal after, as the ask is carried out. The tab
/// drops the Shortcut then, focus on the next one, else on Servers.
#[test]
fn a_removed_shortcut_goes_from_the_tab_as_discord_confirms_it_focus_on_the_next_else_servers() {
    let two = Discord {
        shortcuts: vec![
            shortcut("501", "Late night", Some("Stand-in Server")),
            shortcut("500", "General", Some("Stand-in Server")),
        ],
        ..signed_in()
    };
    let mut dashboard = on_tab(&two);
    press(&mut dashboard, &[Button::Menu]);
    assert_eq!(dashboard.guide().dialog_title(), Some("Late night"));
    press(&mut dashboard, &[A]);
    assert_eq!(asked(&mut dashboard), [Effect::Discord(Ask::RemoveShortcut("501".into()))]);
    // The menu is gone; the Shortcut is there until Discord says.
    assert!(dashboard.guide_line().starts_with("Discord Entry(2) Shortcut \"Late night\""));

    let mut one = two.clone();
    one.shortcuts.remove(0);
    dashboard.set_discord(&one);
    assert_eq!(items(&dashboard), [Item::Account, Item::Idle, Item::Shortcut(0), Item::Servers]);
    assert_eq!(
        dashboard.guide_line(),
        "Discord Entry(2) Shortcut \"General\" (Stand-in Server); not in a call; mute off, deafen off",
        "the next Shortcut"
    );

    // The last one: focus on Servers.
    press(&mut dashboard, &[Button::Menu, A]);
    assert_eq!(asked(&mut dashboard), [Effect::Discord(Ask::RemoveShortcut("500".into()))]);
    dashboard.set_discord(&signed_in());
    assert_eq!(items(&dashboard), [Item::Account, Item::Idle, Item::Servers]);
    assert_eq!(dashboard.guide_line(), "Discord Entry(2) Servers; not in a call; mute off, deafen off");
}

#[test]
fn the_flyout_closes_as_discord_goes() {
    let mut dashboard = on_tab(&signed_in());
    press(&mut dashboard, &[A]);
    dashboard.set_discord(&browsing(servers(&THREE)));
    let gone = Discord { connected: false, user: None, ..signed_in() };
    dashboard.set_discord(&gone);
    assert_eq!(dashboard.flyout(), None);
}

#[test]
fn in_a_call_audio_has_mute_and_deafen_in_discord_after_mute() {
    let call = in_call(&[("2", "Ana")]);
    let mut dashboard = Dashboard::new(screens::fake_library());
    dashboard.set_discord(&call);
    press(&mut dashboard, &[Guide]);
    let entries = dashboard.guide().entries().len();
    press(&mut dashboard, &vec![Down; entries]);
    press(&mut dashboard, &[Right, A]);
    assert_eq!(dashboard.flyout(), Some((Flyout::Audio, FlyoutFocus::Item(0))));
    // Output, Input, Volume, Mute, Mute in Discord, Deafen in Discord.
    press(&mut dashboard, &[Down, Down, Down, Down]);
    asked(&mut dashboard);
    press(&mut dashboard, &[A]);
    assert_eq!(asked(&mut dashboard), [Effect::Discord(Ask::Call(CallAsk::Mute))]);
    press(&mut dashboard, &[Down, A]);
    assert_eq!(asked(&mut dashboard), [Effect::Discord(Ask::Call(CallAsk::Deafen))]);
    // The Call over, focus on Deafen in Discord goes to Mute.
    dashboard.set_discord(&signed_in());
    assert_eq!(dashboard.flyout(), Some((Flyout::Audio, FlyoutFocus::Item(3))));
}

// ------------------------------------------------------------------------------------------------ pictures

fn target_dir() -> PathBuf {
    let from_env = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from);
    from_env.unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"))
}

fn fonts() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let fonts = target_dir().join("fonts");
        assert!(fonts.join("selawk.ttf").is_file(), "no test fonts: run tools/test-fonts.sh");
        std::env::set_var("DASHBOARD_FONTS", fonts);
    });
}

/// Made-up avatars, as the fake Discord's stand-in CDN serves them: one colour with a lighter disc in the middle.
/// Nobody's real avatar.
fn avatar(name: &str, rgb: [u8; 3]) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("discord-avatars");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!("{name}.png"));
    // Made once a run: the tests that draw them run side by side in one process.
    static MADE: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
    let mut made = MADE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if made.contains(&file) {
        return file;
    }
    made.push(file.clone());
    let picture = image::RgbImage::from_fn(64, 64, |x, y| {
        let (dx, dy) = (x as f32 - 31.5, y as f32 - 31.5);
        let light = |c: u8| c.saturating_add(90);
        if dx * dx + dy * dy < 14.0 * 14.0 {
            image::Rgb(rgb.map(light))
        } else {
            image::Rgb(rgb)
        }
    });
    shots::save_shared(&picture, &file);
    file
}

static DRAWING: Mutex<()> = Mutex::new(());

const HD: (u32, u32) = (1920, 1080);

struct Stage {
    gpu: Gpu,
    host: Host,
    target: Offscreen,
    dashboard: Dashboard,
    _drawing: MutexGuard<'static, ()>,
}

impl Stage {
    fn on_tab(discord: &Discord) -> Self {
        fonts();
        let drawing = DRAWING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let gpu = gpu::init(gpu::instance(None), None);
        let host = Host::new(&gpu, FORMAT, || {});
        let target = Offscreen::new(&gpu, HD);
        let mut dashboard = on_tab(discord);
        dashboard.set_clock(Some("9:41 AM".into()));
        Self { gpu, host, target, dashboard, _drawing: drawing }
    }

    fn check(&mut self, name: &str) {
        self.host.store.set_time(10.0);
        let Self { gpu, host, target, dashboard, .. } = self;
        // Twice: the avatars are asked for on the first, and are there by the second's settled frames.
        target.still(gpu, host, |ui| dashboard.draw_all(ui));
        let pixels = target.still(gpu, host, |ui| dashboard.draw_all(ui));
        let image = image::RgbaImage::from_raw(target.size.0, target.size.1, pixels).unwrap();
        let got = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.png"));
        image.save(&got).unwrap();
        shots::check(name, &got, &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference"));
    }
}

/// A Call of four: the user, with Discord's default avatar; Ana speaking; Bo, whose avatar has not come yet,
/// muted; Cy, with a long name, muted and deafened.
fn four() -> Discord {
    let mut call = in_call(&[("2", "Ana"), ("3", "Bo"), ("4", "Cyrus the Unnecessarily Long-Named")]);
    let people = people(&mut call);
    people[0].avatar = Some(avatar("embed-0", [0x58, 0x65, 0xF2]));
    people[1].avatar = Some(avatar("ana", [0xC0, 0x50, 0x40]));
    people[1].speaking = true;
    people[2].muted = true;
    people[3].avatar = Some(avatar("cy", [0x30, 0x90, 0x70]));
    (people[3].muted, people[3].deafened) = (true, true);
    call
}

#[test]
#[ignore = "screenshot"]
fn discord_tab_call() {
    Stage::on_tab(&four()).check("discord-tab-call");
}

/// Five Shortcuts, two of them DMs, focus on the first: made-up pictures for three, the others' stand-ins.
#[test]
#[ignore = "screenshot"]
fn discord_tab_shortcuts() {
    let mut discord = five_shortcuts();
    discord.shortcuts[0].picture = Some(avatar("icon-stand-in", [0x20, 0x60, 0xA0]));
    discord.shortcuts[1].picture = Some(avatar("ana", [0xC0, 0x50, 0x40]));
    discord.shortcuts[4].picture = Some(avatar("icon-stand-in", [0x20, 0x60, 0xA0]));
    Stage::on_tab(&discord).check("discord-tab-shortcuts");
}

/// The Servers Flyout while Discord is asked: its spinner.
#[test]
#[ignore = "screenshot"]
fn discord_servers_loading() {
    let mut stage = Stage::on_tab(&signed_in());
    press(&mut stage.dashboard, &[A]);
    stage.dashboard.set_discord(&browsing(Browse::Servers { loading: true, servers: vec![] }));
    stage.check("discord-servers-loading");
}

/// The servers, those with an icon wearing a made-up one, focus on the second.
#[test]
#[ignore = "screenshot"]
fn discord_servers() {
    let mut stage = Stage::on_tab(&signed_in());
    press(&mut stage.dashboard, &[A]);
    let Browse::Servers { mut servers, .. } = servers(&THREE) else { unreachable!() };
    servers[1].icon = Some(avatar("icon-art", [0xA0, 0x40, 0xA0]));
    servers[2].icon = Some(avatar("icon-stand-in", [0x20, 0x60, 0xA0]));
    stage.dashboard.set_discord(&browsing(Browse::Servers { loading: false, servers }));
    press(&mut stage.dashboard, &[Down]);
    stage.check("discord-servers");
}

/// A server's voice channels: one with three people in it, one Empty, one Discord hasn't said of yet.
#[test]
#[ignore = "screenshot"]
fn discord_server_channels() {
    let mut stage = Stage::on_tab(&signed_in());
    press(&mut stage.dashboard, &[A]);
    stage.dashboard.set_discord(&browsing(servers(&THREE)));
    press(&mut stage.dashboard, &[Down, Down, A]);
    let list = [("500", "General", Some(3)), ("501", "Late night", Some(0)), ("510", "Town hall", None)];
    let mut browse = channels(("901", "Stand-in Server"), &list);
    if let Browse::Server { channels, .. } = &mut browse {
        let faces = [avatar("ana", [0xC0, 0x50, 0x40]), avatar("cy", [0x30, 0x90, 0x70])];
        channels[0].occupants = Some(vec![Some(faces[0].clone()), None, Some(faces[1].clone())]);
    }
    stage.dashboard.set_discord(&browsing(browse));
    stage.check("discord-server-channels");
}

/// Audio in a Call: Mute in Discord on, as Discord said; the Status area's mark badged muted.
#[test]
#[ignore = "screenshot"]
fn discord_audio_in_a_call() {
    let mut call = four();
    call.voice.mute = true;
    let mut stage = Stage::on_tab(&call);
    press(&mut stage.dashboard, &[Button::LeftBumper]);
    let entries = stage.dashboard.guide().entries().len();
    press(&mut stage.dashboard, &vec![Down; entries]);
    press(&mut stage.dashboard, &[Right, A, Down, Down, Down, Down]);
    stage.check("discord-audio-in-call");
}

/// Home's top bar in a Call, the user speaking: the mark lit, left of the clock.
#[test]
#[ignore = "screenshot"]
fn discord_home_mark_speaking() {
    let mut call = four();
    people(&mut call)[0].speaking = true;
    let mut stage = Stage::on_tab(&call);
    press(&mut stage.dashboard, &[B]);
    stage.check("discord-home-mark");
}

/// Muted and deafened, as Discord said: both switches on, focus on Leave.
#[test]
#[ignore = "screenshot"]
fn discord_tab_call_muted_and_deafened() {
    let mut call = four();
    (call.voice.mute, call.voice.deaf) = (true, true);
    (people(&mut call)[0].muted, people(&mut call)[0].deafened) = (true, true);
    let mut stage = Stage::on_tab(&call);
    press(&mut stage.dashboard, &[Down, Down]);
    stage.check("discord-tab-call-muted-deafened");
}
