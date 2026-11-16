//! The Guide's Discord tab (docs/spec/discord-tab.md): what it lists as Discord says, where focus goes as that
//! changes, Mute, Deafen and Leave asked of Discord and shown done only once Discord says so, and what each state
//! looks like. `SHOTS=bless cargo test -p screens --test discord`.
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use model::discord::{Ask, Call, CallAsk, Discord, Person, Voice};
use render::gpu::{self, Gpu};
use render::offscreen::FORMAT;
use screens::guide::{DiscordItem as Item, Focus, Kind, Tab};
use screens::{Dashboard, Effect};
use ui::host::Offscreen;
use ui::Button::{self, Down, Guide, RightBumper, Up, A, B};
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
    Discord {
        connected: true,
        has_tokens: true,
        has_secret: true,
        client_id: Some("1".into()),
        user: Some("E2E Tester".into()),
        status: "Connected as E2E Tester".into(),
        ..Discord::default()
    }
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
fn while_it_cant_show_discord_the_tab_says_why_and_nothing_else() {
    let lost = Discord { connected: false, retrying: true, user: None, ..signed_in() };
    let mut dashboard = on_tab(&lost);
    assert_eq!(items(&dashboard), [Item::Unavailable]);
    assert_eq!(dashboard.guide().entries()[0].title, "Discord isn't running");
    press(&mut dashboard, &[Down, A]);
    assert_eq!(focus(&dashboard), (Tab::Discord, Focus::Entry(0)));
    assert!(asked(&mut dashboard).is_empty());
    assert_eq!(dashboard.guide_line(), "Discord Entry(0) Unavailable; \"Discord isn't running\"");

    for (discord, line) in [
        (Discord { connecting: true, ..lost.clone() }, "Connecting to Discord"),
        (Discord { needs_reconnect: true, ..lost.clone() }, "Reconnect Discord in settings"),
    ] {
        dashboard.set_discord(&discord);
        assert_eq!(dashboard.guide().entries()[0].title, line);
    }
}

#[test]
fn out_of_a_call_it_shows_who_is_signed_in_and_a_does_nothing() {
    let mut dashboard = on_tab(&signed_in());
    assert_eq!(items(&dashboard), [Item::Account, Item::Idle]);
    assert_eq!(dashboard.guide().entries()[1].title, "You're not in a voice channel");
    assert_eq!(focus(&dashboard), (Tab::Discord, Focus::Entry(0)));
    press(&mut dashboard, &[A, Down, A]);
    assert!(asked(&mut dashboard).is_empty() && dashboard.guide_open());
    assert_eq!(dashboard.guide_line(), "Discord Entry(1) Idle; not in a call; mute off, deafen off");
}

#[test]
fn in_a_call_it_lists_the_call_then_mute_deafen_and_leave_with_focus_on_mute() {
    let mut dashboard = on_tab(&in_call(&[("2", "Ana"), ("3", "Bo")]));
    let call =
        [Item::Channel, Item::Person(0), Item::Person(1), Item::Person(2), Item::Mute, Item::Deafen, Item::Leave];
    assert_eq!(items(&dashboard), call);
    assert_eq!(focus(&dashboard), (Tab::Discord, Focus::Entry(4)));
    let titles: Vec<&str> = dashboard.guide().entries().iter().map(|entry| entry.title.as_str()).collect();
    assert_eq!(titles, ["General", "E2E Tester", "Ana", "Bo", "Mute", "Deafen", "Leave"]);
    // A on the Call's heading or someone in it does nothing.
    press(&mut dashboard, &[Up, A, Up, Up, Up, A]);
    assert_eq!(focus(&dashboard), (Tab::Discord, Focus::Entry(0)));
    assert!(asked(&mut dashboard).is_empty());
    // Nothing under Leave: down from it stays.
    press(&mut dashboard, &[Down, Down, Down, Down, Down, Down, Down]);
    assert_eq!(focus(&dashboard), (Tab::Discord, Focus::Entry(6)));
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

    // Out of the Call: focus starts afresh.
    dashboard.set_discord(&signed_in());
    assert_eq!(items(&dashboard), [Item::Account, Item::Idle]);
    assert_eq!(focus(&dashboard), (Tab::Discord, Focus::Entry(0)));
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
fn discord_going_mid_call_leaves_the_tab_saying_so_and_disconnect_takes_the_tab_away() {
    let mut dashboard = on_tab(&in_call(&[("2", "Ana")]));
    let gone = Discord { connected: false, retrying: true, user: None, voice: Voice::default(), ..signed_in() };
    dashboard.set_discord(&gone);
    assert_eq!(items(&dashboard), [Item::Unavailable]);
    assert_eq!(focus(&dashboard), (Tab::Discord, Focus::Entry(0)));
    dashboard.set_discord(&Discord::default());
    assert_eq!(dashboard.guide().tabs(), [Tab::Landing, Tab::ProfileAndSystem]);
    assert_eq!(focus(&dashboard).0, Tab::Landing);
}

#[test]
fn the_tab_is_there_from_connect_until_disconnect_discord_running_or_not() {
    let mut dashboard = Dashboard::new(screens::fake_library());
    // Discord out of reach, with tokens saved: the tab is there, and says why.
    dashboard.set_discord(&Discord { connected: false, retrying: true, user: None, ..signed_in() });
    press(&mut dashboard, &[Guide]);
    assert_eq!(dashboard.guide().tabs(), [Tab::Landing, Tab::Discord, Tab::ProfileAndSystem]);
    press(&mut dashboard, &[B]);
    // Never connected: none.
    dashboard.set_discord(&Discord { client_id: Some("1".into()), has_secret: true, ..Discord::default() });
    press(&mut dashboard, &[Guide]);
    assert_eq!(dashboard.guide().tabs(), [Tab::Landing, Tab::ProfileAndSystem]);
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
fn discord_tab_not_running() {
    let lost = Discord { connected: false, retrying: true, user: None, ..signed_in() };
    Stage::on_tab(&lost).check("discord-tab-not-running");
}

#[test]
fn discord_tab_reconnect() {
    let lost = Discord { connected: false, needs_reconnect: true, user: None, ..signed_in() };
    Stage::on_tab(&lost).check("discord-tab-reconnect");
}

#[test]
fn discord_tab_call() {
    Stage::on_tab(&four()).check("discord-tab-call");
}

/// Muted and deafened, as Discord said: both switches on, focus on Leave.
#[test]
fn discord_tab_call_muted_and_deafened() {
    let mut call = four();
    (call.voice.mute, call.voice.deaf) = (true, true);
    (people(&mut call)[0].muted, people(&mut call)[0].deafened) = (true, true);
    let mut stage = Stage::on_tab(&call);
    press(&mut stage.dashboard, &[Down, Down]);
    stage.check("discord-tab-call-muted-deafened");
}
