//! Discord's Direct Messages in the Guide (docs/spec/party-chats.md): the tab's section, All Direct Messages and a
//! conversation, drawn from fake data and moved through with the pad; what is asked of the web session; and that no
//! name or word of a message reaches a log line. `SHOTS=bless cargo test -p screens --test direct`.
//!
//! Every name here is made up (Ana, Bo, Cy ...). "Now" is fixed: 2026-11-18, 21:45 in UTC.
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use model::discord::{Call, Discord, Person, Shortcut as Short, Voice};
use model::discord_dms::{
    Body, Conversation, Delivery, Direct, DirectAsk, DirectEvent, DirectState, Dm, Emoji, Extra, Id, LineKind, Message,
    Picture, Reaction, Reply, Ring, Span,
};
use render::gpu::{self, Gpu};
use render::offscreen::FORMAT;
use screens::guide::{DiscordItem as Item, Focus, Kind, Tab};
use screens::keyboard::Typed;
use screens::{Dashboard, Effect};
use ui::host::Offscreen;
use ui::Button::{self, Down, Guide, Left, LeftTrigger, Menu, Right, RightBumper, RightTrigger, Up, A, B, X, Y};
use ui::{Host, Press};

const NOW: i64 = 1_795_038_300_000;
const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;

fn press(dashboard: &mut Dashboard, buttons: &[Button]) {
    for &button in buttons {
        dashboard.press(Press { button, repeat: false });
    }
}

/// What has been asked of the shell, but for the sounds.
fn asked(dashboard: &mut Dashboard) -> Vec<Effect> {
    dashboard.take_effects().into_iter().filter(|effect| !matches!(effect, Effect::Sound(_))).collect()
}

/// Only what has been asked of the web session.
fn asks(dashboard: &mut Dashboard) -> Vec<DirectAsk> {
    asked(dashboard)
        .into_iter()
        .filter_map(|effect| match effect {
            Effect::Direct(ask) => Some(ask),
            _ => None,
        })
        .collect()
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

/// The place focus is on, as an item of the tab.
fn on(dashboard: &Dashboard) -> Option<Item> {
    let Focus::Entry(at) = focus(dashboard).1 else { return None };
    match dashboard.guide().entries().get(at)?.kind {
        Kind::Discord(item) => Some(item),
        _ => None,
    }
}

// ------------------------------------------------------------------------------------------------ fake data

/// A made-up avatar, encoded as the session would give it: a colour with a lighter disc in the middle.
fn picture(rgb: [u8; 3]) -> Picture {
    let image = image::RgbImage::from_fn(64, 64, |x, y| {
        let (dx, dy) = (x as f32 - 31.5, y as f32 - 31.5);
        let light = |c: u8| c.saturating_add(90);
        if dx * dx + dy * dy < 14.0 * 14.0 {
            image::Rgb(rgb.map(light))
        } else {
            image::Rgb(rgb)
        }
    });
    let mut bytes = Vec::new();
    image.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
    Picture(Arc::from(bytes))
}

fn dm(id: &str, name: &str, unread: u32, last: Option<(&str, i64)>) -> Dm {
    Dm {
        id: id.into(),
        name: name.into(),
        group: false,
        members: 2,
        picture: None,
        unread,
        last: last.map(|(text, at)| (text.into(), at)),
    }
}

fn group(id: &str, name: &str, members: u32, unread: u32, last: Option<(&str, i64)>) -> Dm {
    Dm { group: true, members, ..dm(id, name, unread, last) }
}

/// Seven DMs: Ana (3 unread) and Cy's Group DM (1) have unread; the first three have pictures.
fn seven() -> Vec<Dm> {
    let mut dms = vec![
        dm("d1", "Ana", 3, Some(("are you on tonight?", NOW - 20 * MINUTE))),
        dm("d2", "Bo", 0, Some(("gg, that last round was close", NOW - DAY - 2 * HOUR))),
        group("d3", "Raid night", 4, 1, Some(("Cy: bring snacks", NOW - 3 * DAY))),
        dm("d4", "Di", 0, None),
        dm("d5", "Ed", 0, Some(("ok", NOW - 9 * DAY))),
        dm("d6", "Flo", 0, None),
        dm("d7", "Gus", 0, None),
    ];
    dms[0].picture = Some(picture([0xC0, 0x50, 0x40]));
    dms[1].picture = Some(picture([0x30, 0x90, 0x70]));
    dms[2].picture = Some(picture([0x20, 0x60, 0xA0]));
    dms
}

fn ready(dms: Vec<Dm>) -> Direct {
    Direct { state: DirectState::Ready, dms, ..Direct::default() }
}

fn state(state: DirectState) -> Direct {
    Direct { state, ..Direct::default() }
}

fn text(words: &str) -> Vec<Span> {
    vec![Span::Text(words.into())]
}

fn said(id: &str, own: bool, author: &str, at: i64, words: &str) -> Message {
    Message {
        id: id.into(),
        author: if own { "me".into() } else { Id(format!("u-{author}")) },
        author_name: author.into(),
        author_picture: None,
        own,
        at,
        edited: false,
        body: Body::Said { text: text(words), reply: None, extras: Vec::new(), reactions: Vec::new() },
        delivery: Delivery::Sent,
    }
}

fn with(
    mut message: Message,
    change: impl FnOnce(&mut Vec<Span>, &mut Option<Reply>, &mut Vec<Extra>, &mut Vec<Reaction>),
) -> Message {
    if let Body::Said { text, reply, extras, reactions } = &mut message.body {
        change(text, reply, extras, reactions);
    }
    message
}

fn reaction(emoji: &str, count: u32, mine: bool) -> Reaction {
    Reaction { emoji: Emoji::Unicode(emoji.into()), count, mine }
}

/// A conversation with Ana: yesterday's talk, then today's, three of Ana's unread (the unread line before the
/// first), a reply, reactions, an image, a link and a mention, one of Discord's own lines, and one of the user's own.
fn with_ana() -> Conversation {
    let yesterday = NOW - DAY;
    let mut messages = vec![
        said("m1", false, "Ana", yesterday - 5 * MINUTE, "did you see the new map?"),
        said("m2", true, "me", yesterday - 4 * MINUTE, "not yet, any good?"),
        with(said("m3", false, "Ana", yesterday - 2 * MINUTE, "so good"), |_, _, _, reactions| {
            reactions.push(reaction("\u{1F44D}", 2, true));
            reactions.push(reaction("\u{1F602}", 1, false));
        }),
        Message {
            body: Body::Line { kind: LineKind::CallStarted, text: "Ana started a call".into() },
            ..said("m4", false, "Ana", yesterday, "")
        },
        said("m5", false, "Ana", NOW - 40 * MINUTE, "are you on tonight?"),
        with(said("m6", false, "Ana", NOW - 39 * MINUTE, "bring Bo too, @Bo"), |text, _, _, _| {
            *text = vec![
                Span::Text("bring ".into()),
                Span::Mention("@Bo".into()),
                Span::Text(" too, details at ".into()),
                Span::Link("example.test/raid".into()),
            ];
        }),
        with(said("m7", false, "Ana", NOW - 38 * MINUTE, "look at this"), |_, _, extras, _| {
            extras.push(Extra::Images(1));
        }),
        with(said("m8", true, "me", NOW - 30 * MINUTE, "after food, 20 min"), |_, reply, _, _| {
            *reply = Some(Reply::To { author: "Ana".into(), line: "are you on tonight?".into() });
        }),
    ];
    messages[7].edited = true;
    Conversation {
        id: "d1".into(),
        name: "Ana".into(),
        group: false,
        picture: Some(picture([0xC0, 0x50, 0x40])),
        messages,
        loading_older: false,
        at_start: true,
        unread_from: Some("m5".into()),
    }
}

fn open_with(direct: &Direct, conversation: Conversation) -> Direct {
    Direct { open: Some(conversation), ..direct.clone() }
}

fn voice_in_a_call() -> Discord {
    let me = Person {
        id: "1".into(),
        name: "E2E Tester".into(),
        avatar: None,
        speaking: false,
        muted: false,
        deafened: false,
        me: true,
    };
    let ana = Person {
        id: "2".into(),
        name: "Ana".into(),
        avatar: None,
        speaking: true,
        muted: false,
        deafened: false,
        me: false,
    };
    let call = Call { id: "500".into(), title: "General".into(), people: vec![me, ana] };
    Discord {
        connected: true,
        has_tokens: true,
        has_secret: true,
        client_id: Some("1".into()),
        user: Some("E2E Tester".into()),
        voice: Voice { call: Some(call), mute: false, deaf: false },
        ..Discord::default()
    }
}

fn voice_idle() -> Discord {
    Discord { voice: Voice::default(), ..voice_in_a_call() }
}

/// The Guide open on the Discord tab, the time fixed.
fn on_tab(discord: &Discord, direct: &Direct) -> Dashboard {
    let mut dashboard = Dashboard::new(screens::fake_library());
    dashboard.set_time(Some((NOW, 0)));
    dashboard.set_discord(discord);
    dashboard.set_direct(direct);
    press(&mut dashboard, &[Guide, RightBumper]);
    dashboard.take_effects();
    assert_eq!(focus(&dashboard).0, Tab::Discord);
    dashboard
}

fn direct_only(direct: &Direct) -> Dashboard {
    on_tab(&Discord::default(), direct)
}

// ------------------------------------------------------------------------------------------------ the tab

#[test]
fn the_tab_is_there_when_direct_messages_are_on_or_the_voice_connection_is() {
    let mut dashboard = Dashboard::new(screens::fake_library());
    press(&mut dashboard, &[Guide]);
    assert_eq!(dashboard.guide().tabs(), [Tab::Landing, Tab::ProfileAndSystem]);
    dashboard.set_direct(&state(DirectState::Loading));
    assert_eq!(dashboard.guide().tabs(), [Tab::Landing, Tab::Discord, Tab::ProfileAndSystem]);
    dashboard.set_direct(&state(DirectState::Off));
    assert_eq!(dashboard.guide().tabs(), [Tab::Landing, Tab::ProfileAndSystem]);
    // The voice connection alone is enough, as it was.
    dashboard.set_discord(&voice_idle());
    assert_eq!(dashboard.guide().tabs(), [Tab::Landing, Tab::Discord, Tab::ProfileAndSystem]);
}

#[test]
fn with_direct_messages_on_the_tab_lists_the_discs_then_the_sixth_circle_and_focus_is_on_the_first_dm() {
    let dashboard = direct_only(&ready(seven()));
    use Item::*;
    assert_eq!(items(&dashboard), [DM(0), DM(1), DM(2), DM(3), DM(4), AllDMs]);
    assert_eq!(focus(&dashboard), (Tab::Discord, Focus::Entry(0)));
    // Places and counts, never names.
    assert_eq!(
        dashboard.guide_line(),
        "Discord Entry(0) DM(0) of 7, unread 3; not in a call; mute off, deafen off; dms 7, unread 4 in 2"
    );
    // No account entry and no "not in a voice channel" with Direct Messages on.
    assert!(!items(&dashboard).contains(&Account) && !items(&dashboard).contains(&Idle));
}

#[test]
fn the_sections_stand_in_the_specs_order_answer_the_call_the_voice_line_direct_messages_then_shortcuts_and_servers() {
    use Item::*;
    let mut direct = ready(seven());
    direct.rings = vec![Ring { channel: "d2".into(), caller: "Bo".into(), picture: None }];
    let mut voice = voice_in_a_call();
    voice.shortcuts =
        vec![Short { id: "600".into(), name: "Late night".into(), server: None, picture: None, dm: false }];
    let dashboard = on_tab(&voice, &direct);
    assert_eq!(
        items(&dashboard),
        [
            Answer,
            Channel,
            Person(0),
            Person(1),
            Mute,
            Deafen,
            Leave,
            DM(0),
            DM(1),
            DM(2),
            DM(3),
            DM(4),
            AllDMs,
            Shortcut(0),
            Servers
        ]
    );
    // Ringing: Answer has the first focus.
    assert_eq!(on(&dashboard), Some(Answer));
    assert_eq!(dashboard.guide_line(), "Discord Entry(0) Answer; ringing");
    // The section's first entry is under a divider when anything is above it, and Shortcuts under one of their own.
    let entries = dashboard.guide().entries();
    assert!(entries[7].divider && entries[13].divider);

    // In a Call without a ring: Mute.
    let in_call = on_tab(&voice, &ready(seven()));
    assert_eq!(on(&in_call), Some(Mute));
    // Voice can't show: its line, then Direct Messages, and no Shortcuts or Servers.
    let lost = Discord { connected: false, retrying: true, ..voice_idle() };
    let dashboard = on_tab(&lost, &ready(seven()));
    assert_eq!(items(&dashboard), [Unavailable, DM(0), DM(1), DM(2), DM(3), DM(4), AllDMs]);
    // Never set up, Direct Messages on: the section alone.
    let dashboard = direct_only(&ready(seven()));
    assert_eq!(items(&dashboard)[0], DM(0));
}

#[test]
fn up_to_five_dms_then_the_sixth_circle_which_has_focus_with_none() {
    use Item::*;
    let three = direct_only(&ready(seven()[..3].to_vec()));
    assert_eq!(items(&three), [DM(0), DM(1), DM(2), AllDMs]);
    let dashboard = direct_only(&ready(Vec::new()));
    assert_eq!(items(&dashboard), [DMsState(screens::guide::DirectNote::Empty)]);
    assert_eq!(dashboard.guide().entries()[0].title, "No Direct Messages yet");
    assert_eq!(dashboard.guide_line(), "Discord Entry(0) DMsState(Empty)");
}

#[test]
fn left_and_right_move_along_the_row_held_at_its_ends_and_down_and_up_leave_it() {
    use Item::*;
    let mut voice = voice_idle();
    voice.shortcuts = vec![
        Short { id: "600".into(), name: "Late night".into(), server: None, picture: None, dm: false },
        Short { id: "601".into(), name: "Raid".into(), server: None, picture: None, dm: false },
    ];
    let mut dashboard = on_tab(&voice, &ready(seven()));
    assert_eq!(on(&dashboard), Some(DM(0)));
    press(&mut dashboard, &[Left]);
    assert_eq!(on(&dashboard), Some(DM(0)), "held at the row's start");
    press(&mut dashboard, &[Right, Right]);
    assert_eq!(on(&dashboard), Some(DM(2)));
    // Down from anywhere on the row goes to the entry under the section: the first Shortcut.
    press(&mut dashboard, &[Down]);
    assert_eq!(on(&dashboard), Some(Shortcut(0)));
    // Up from it lands on the disc focus last left.
    press(&mut dashboard, &[Up]);
    assert_eq!(on(&dashboard), Some(DM(2)));
    press(&mut dashboard, &[Right, Right, Right, Right]);
    assert_eq!(on(&dashboard), Some(AllDMs), "held at the sixth circle");
    press(&mut dashboard, &[Down, Up]);
    assert_eq!(on(&dashboard), Some(AllDMs));
    // Up from the row is the entry above it; at the tab's top, nothing.
    press(&mut dashboard, &[Up]);
    assert_eq!(on(&dashboard), Some(AllDMs));
}

#[test]
fn focus_stays_on_the_same_conversation_as_the_list_changes() {
    use Item::*;
    let mut dashboard = direct_only(&ready(seven()));
    press(&mut dashboard, &[Right, Right]);
    assert_eq!(on(&dashboard), Some(DM(2)), "Raid night");
    // A new conversation comes in first: Raid night moves along to the fourth disc, focus with it.
    let mut dms = seven();
    dms.insert(0, dm("d0", "New Person", 1, None));
    dashboard.set_direct(&ready(dms.clone()));
    assert_eq!(on(&dashboard), Some(DM(3)));
    // Pushed off the row (the sixth DM now): onto the sixth circle.
    dms.insert(0, dm("d00", "Another", 0, None));
    dms.insert(0, dm("d01", "More", 0, None));
    dashboard.set_direct(&ready(dms.clone()));
    assert_eq!(on(&dashboard), Some(AllDMs));
    // A message arriving (an unread count changing) moves nothing.
    dms[3].unread += 5;
    let before = focus(&dashboard);
    dashboard.set_direct(&ready(dms));
    assert_eq!(focus(&dashboard), before);
}

#[test]
fn the_section_says_why_it_cant_show_the_list() {
    use screens::guide::DirectNote as Note;
    for (direct, note, line, log) in [
        (state(DirectState::Loading), Note::Loading, "Loading Direct Messages", "Loading"),
        (
            state(DirectState::SignedOut),
            Note::SignedOut,
            "Sign in to Discord in Settings > Account > Direct Messages",
            "SignedOut",
        ),
        (
            state(DirectState::NoRuntime),
            Note::NoRuntime,
            "Messages need Microsoft Edge WebView2, which isn't on this PC",
            "NoRuntime",
        ),
        (state(DirectState::Unreachable), Note::Unreachable, "Can't reach Discord. Trying again", "Unreachable"),
        (
            state(DirectState::PageChanged),
            Note::PageChanged,
            "Discord has changed its page, so Direct Messages can't be read until the Dashboard is updated",
            "PageChanged",
        ),
        (ready(Vec::new()), Note::Empty, "No Direct Messages yet", "Empty"),
    ] {
        let mut dashboard = direct_only(&direct);
        assert_eq!(items(&dashboard), [Item::DMsState(note)]);
        assert_eq!(dashboard.guide().entries()[0].title, line);
        assert_eq!(dashboard.guide_line(), format!("Discord Entry(0) DMsState({log})"));
        // A does nothing but on "Sign in".
        press(&mut dashboard, &[A]);
        assert!(asked(&mut dashboard).is_empty() || note == Note::SignedOut);
    }
}

#[test]
fn a_on_sign_in_closes_the_guide_and_opens_settings_there() {
    let mut dashboard = direct_only(&state(DirectState::SignedOut));
    press(&mut dashboard, &[A]);
    assert!(!dashboard.guide_open());
    assert_eq!(dashboard.page(), Some("Settings"));
}

#[test]
fn the_unread_dot_is_on_the_tabs_mark_while_any_dm_is_unread() {
    let dashboard = direct_only(&ready(seven()));
    assert_eq!(dashboard.guide().direct().unread(), (4, 2));
    let none = direct_only(&ready(seven()[3..].to_vec()));
    assert_eq!(none.guide().direct().unread(), (0, 0));
}

#[test]
fn turning_direct_messages_off_forgets_everything() {
    let mut dashboard = direct_only(&ready(seven()));
    press(&mut dashboard, &[A]);
    dashboard.set_direct(&open_with(&ready(seven()), with_ana()));
    press(&mut dashboard, &[A]);
    dashboard.take_effects();
    dashboard.set_direct(&state(DirectState::Off));
    assert!(!dashboard.guide().direct_open());
    assert!(dashboard.guide().dms().drafts.is_empty());
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

static DRAWING: Mutex<()> = Mutex::new(());

const HD: (u32, u32) = (1920, 1080);

/// A Dashboard drawn offscreen, for what a frame decides (where a conversation opens) and for the pictures.
struct Stage {
    gpu: Gpu,
    host: Host,
    target: Offscreen,
    dashboard: Dashboard,
    _drawing: MutexGuard<'static, ()>,
}

impl Stage {
    fn new(dashboard: Dashboard) -> Self {
        fonts();
        let drawing = DRAWING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let gpu = gpu::init(gpu::instance(None), None);
        let host = Host::new(&gpu, FORMAT, || {});
        let target = Offscreen::new(&gpu, HD);
        let mut stage = Self { gpu, host, target, dashboard, _drawing: drawing };
        stage.dashboard.set_clock(Some("9:41 PM".into()));
        stage
    }

    /// One frame, settled: the pictures it asks for are decoded and it is drawn again.
    fn frame(&mut self) -> image::RgbaImage {
        self.host.store.set_time(10.0);
        let Self { gpu, host, target, dashboard, .. } = self;
        target.still(gpu, host, |ui| dashboard.draw_all(ui));
        let pixels = target.still(gpu, host, |ui| dashboard.draw_all(ui));
        image::RgbaImage::from_raw(target.size.0, target.size.1, pixels).unwrap()
    }

    fn check(&mut self, name: &str) {
        let image = self.frame();
        let got = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.png"));
        image.save(&got).unwrap();
        shots::check(name, &got, &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference"));
    }

    /// Draws a frame: a conversation that has come is placed by it.
    fn settle(&mut self) {
        self.frame();
    }
}

// ------------------------------------------------------------------------------------------------ conversations

/// The tab on Ana, her conversation asked for and come.
fn in_a_conversation(conversation: Conversation) -> Stage {
    let direct = ready(seven());
    let mut dashboard = direct_only(&direct);
    press(&mut dashboard, &[A]);
    dashboard.set_direct(&open_with(&direct, conversation));
    dashboard.take_effects();
    let mut stage = Stage::new(dashboard);
    stage.settle();
    stage
}

fn line(dashboard: &Dashboard) -> String {
    let line = dashboard.guide_line();
    line.split(", flyout ").nth(1).unwrap_or_default().to_string()
}

#[test]
fn a_on_a_disc_opens_the_conversation_asked_for_and_loading_until_the_session_has_it() {
    let direct = ready(seven());
    let mut dashboard = direct_only(&direct);
    press(&mut dashboard, &[A]);
    assert_eq!(asks(&mut dashboard), [DirectAsk::Open("d1".into())]);
    assert_eq!(line(&dashboard), "Conversation loading");
    // Another conversation's messages are not this one's.
    let mut other = with_ana();
    other.id = "d2".into();
    dashboard.set_direct(&open_with(&direct, other));
    assert_eq!(line(&dashboard), "Conversation loading");
    dashboard.set_direct(&open_with(&direct, with_ana()));
    assert_eq!(line(&dashboard), "Conversation ReplyBox; draft 0 chars");
    // B closes it: asked, and focus is back on the DM.
    press(&mut dashboard, &[B]);
    assert_eq!(asks(&mut dashboard), [DirectAsk::Close]);
    assert_eq!(on(&dashboard), Some(Item::DM(0)));
    assert!(!dashboard.guide().direct_open());
}

#[test]
fn opening_puts_focus_on_the_reply_box_when_the_unread_fit_and_walking_up_goes_a_message_at_a_time() {
    let stage = in_a_conversation(with_ana());
    let mut dashboard = stage.dashboard;
    assert_eq!(line(&dashboard), "Conversation ReplyBox; draft 0 chars");
    // Up: the newest message (the user's own), theirs, and on; Discord's own line is not a stop.
    press(&mut dashboard, &[Up]);
    assert_eq!(line(&dashboard), "Conversation Message(0) of 7, own");
    press(&mut dashboard, &[Up, Up]);
    assert_eq!(line(&dashboard), "Conversation Message(2) of 7, theirs");
    press(&mut dashboard, &[Up]);
    assert_eq!(line(&dashboard), "Conversation Message(3) of 7, theirs, unread line above");
    press(&mut dashboard, &[Up, Up]);
    assert_eq!(line(&dashboard), "Conversation Message(5) of 7, own");
    // Down on the way back; Down from the newest is the reply box, and Down there does nothing.
    press(&mut dashboard, &[Down, Down, Down, Down, Down, Down]);
    assert_eq!(line(&dashboard), "Conversation ReplyBox; draft 0 chars");
    press(&mut dashboard, &[Down]);
    assert_eq!(line(&dashboard), "Conversation ReplyBox; draft 0 chars");
}

/// Twenty messages of Ana's, all of them unread: they cannot all be in view.
fn many_unread() -> Conversation {
    let messages = (0..20)
        .map(|n| {
            said(
                &format!("u{n}"),
                false,
                "Ana",
                NOW - 90 * MINUTE + n * MINUTE,
                &format!(
                    "message number {n} of the lot, and a bit longer so it wraps over a line or two of the bubble"
                ),
            )
        })
        .collect();
    Conversation { unread_from: Some("u3".into()), at_start: true, messages, ..with_ana() }
}

#[test]
fn opening_with_more_unread_than_fit_puts_focus_on_the_first_unread_message() {
    let stage = in_a_conversation(many_unread());
    let dashboard = stage.dashboard;
    assert_eq!(line(&dashboard), "Conversation Message(16) of 20, theirs, unread line above");
}

#[test]
fn triggers_page_by_as_many_messages_as_the_view_shows_to_the_oldest_and_the_newest() {
    let mut stage = in_a_conversation(Conversation { unread_from: None, ..many_unread() });
    let shown = stage.dashboard.guide().dms().chat().map(|chat| chat.shown).unwrap();
    assert!(shown >= 3 && shown < 20, "{shown} messages in view");
    let dashboard = &mut stage.dashboard;
    press(dashboard, &[LeftTrigger]);
    assert_eq!(line(dashboard), format!("Conversation Message({}) of 20, theirs", shown.min(20) - 1));
    press(dashboard, &[LeftTrigger, LeftTrigger, LeftTrigger, LeftTrigger, LeftTrigger]);
    assert_eq!(line(dashboard), "Conversation Message(19) of 20, theirs");
    press(dashboard, &[RightTrigger]);
    assert_eq!(line(dashboard), format!("Conversation Message({}) of 20, theirs", 19 - shown));
    for _ in 0..10 {
        if line(dashboard) == "Conversation Message(0) of 20, theirs" {
            break;
        }
        press(dashboard, &[RightTrigger]);
    }
    assert_eq!(line(dashboard), "Conversation Message(0) of 20, theirs");
    press(dashboard, &[RightTrigger]);
    assert_eq!(line(dashboard), "Conversation ReplyBox; draft 0 chars");
}

#[test]
fn older_messages_are_asked_for_once_when_focus_reaches_the_oldest() {
    let conversation = Conversation { at_start: false, unread_from: None, ..with_ana() };
    let stage = in_a_conversation(conversation.clone());
    let mut dashboard = stage.dashboard;
    // Seven stops: up to the oldest.
    press(&mut dashboard, &[Up, Up, Up, Up, Up, Up]);
    assert!(asks(&mut dashboard).is_empty());
    press(&mut dashboard, &[Up]);
    assert_eq!(asks(&mut dashboard), [DirectAsk::Older]);
    // Held, or pressed again, it is not asked twice.
    press(&mut dashboard, &[Up, Up]);
    assert!(asks(&mut dashboard).is_empty());
    // While they load, nothing more; when they come focus stays on the same message.
    let direct = dashboard.direct().clone();
    let mut loading = conversation.clone();
    loading.loading_older = true;
    dashboard.set_direct(&open_with(&direct, loading));
    press(&mut dashboard, &[Up]);
    assert!(asks(&mut dashboard).is_empty());
    let mut longer = conversation.clone();
    longer.messages.insert(0, said("m0", false, "Ana", NOW - DAY - 20 * MINUTE, "older"));
    dashboard.set_direct(&open_with(&direct, longer.clone()));
    assert_eq!(line(&dashboard), "Conversation Message(6) of 8, theirs");
    // Up onto the new oldest asks again; at the start of the conversation never.
    press(&mut dashboard, &[Up]);
    assert_eq!(asks(&mut dashboard), [DirectAsk::Older]);
    longer.at_start = true;
    dashboard.set_direct(&open_with(&direct, longer));
    press(&mut dashboard, &[Up]);
    assert!(asks(&mut dashboard).is_empty());
}

#[test]
fn new_messages_while_open_are_counted_below_while_focus_is_higher_up() {
    let stage = in_a_conversation(with_ana());
    let mut dashboard = stage.dashboard;
    press(&mut dashboard, &[Up, Up, Up]);
    let direct = dashboard.direct().clone();
    let mut longer = with_ana();
    longer.messages.push(said("m9", false, "Ana", NOW - MINUTE, "you there?"));
    longer.messages.push(said("m10", false, "Ana", NOW, "hello?"));
    dashboard.set_direct(&open_with(&direct, longer));
    assert_eq!(line(&dashboard), "Conversation Message(4) of 9, theirs, 2 new below");
    // Down to the reply box takes the count away.
    press(&mut dashboard, &[Down, Down, Down, Down, Down]);
    assert_eq!(line(&dashboard), "Conversation ReplyBox; draft 0 chars");
    press(&mut dashboard, &[Up]);
    assert_eq!(line(&dashboard), "Conversation Message(0) of 9, theirs");
}

#[test]
fn a_message_deleted_under_focus_moves_it_to_the_next_newer() {
    let stage = in_a_conversation(with_ana());
    let mut dashboard = stage.dashboard;
    press(&mut dashboard, &[Up, Up, Up, Up]);
    let direct = dashboard.direct().clone();
    let mut fewer = with_ana();
    fewer.messages.retain(|message| message.id.as_str() != "m5");
    dashboard.set_direct(&open_with(&direct, fewer));
    // m5 is gone: m6, the next newer, has focus.
    assert_eq!(line(&dashboard), "Conversation Message(2) of 6, theirs");
}

// ------------------------------------------------------------------------------------------------ the DM's menu and All Direct Messages

#[test]
fn menu_on_a_disc_is_the_dms_menu_mark_as_read_call_cancel() {
    let mut dashboard = direct_only(&ready(seven()));
    press(&mut dashboard, &[Menu]);
    assert!(
        dashboard.guide_line().ends_with(", dialog Menu { prompt: false, row: 0 } (dm)"),
        "{}",
        dashboard.guide_line()
    );
    // The title is the DM's name, which the log never has.
    assert_eq!(dashboard.guide().dialog_title(), None);
    press(&mut dashboard, &[A]);
    assert_eq!(asks(&mut dashboard), [DirectAsk::MarkRead("d1".into())]);
    assert!(dashboard.guide().dialog().is_none());
    // Call is offered while Direct Messages are Ready.
    press(&mut dashboard, &[Menu, Down, A]);
    assert_eq!(asks(&mut dashboard), [DirectAsk::Call("d1".into())]);
    // A DM with nothing unread has no Mark As Read: its first row is Call.
    press(&mut dashboard, &[Right, Menu, A]);
    assert_eq!(asks(&mut dashboard), [DirectAsk::Call("d2".into())]);
    // Cancel and B close it.
    press(&mut dashboard, &[Menu, Down, A]);
    assert!(asks(&mut dashboard).is_empty() && dashboard.guide().dialog().is_none());
    press(&mut dashboard, &[Menu, B]);
    assert!(asks(&mut dashboard).is_empty() && dashboard.guide().dialog().is_none());
    // Nothing on the sixth circle, nor X, Y, View or the triggers on a disc.
    press(&mut dashboard, &[Right, Right, Right, Right, Menu, X, Y, LeftTrigger]);
    assert!(dashboard.guide().dialog().is_none() && asks(&mut dashboard).is_empty());
}

#[test]
fn all_direct_messages_lists_the_rows_and_a_opens_a_conversation_in_its_place() {
    let mut dashboard = direct_only(&ready(seven()));
    press(&mut dashboard, &[Right, Right, Right, Right, Right, A]);
    // Focus starts on the DM that had focus on the tab (the one the sixth circle was reached from), which is in
    // the list.
    assert_eq!(line(&dashboard), "DirectMessages DM(4) of 7");
    press(&mut dashboard, &[Up, Up]);
    assert_eq!(line(&dashboard), "DirectMessages DM(2) of 7, unread 1, group");
    // Held at the ends.
    press(&mut dashboard, &[Down, Down, Down, Down, Down, Down, Down]);
    assert_eq!(line(&dashboard), "DirectMessages DM(6) of 7");
    press(&mut dashboard, &[Up, Up, Up, Up, Up, Up, Up, Up]);
    assert_eq!(line(&dashboard), "DirectMessages DM(0) of 7, unread 3");
    // Menu opens the DM's menu.
    press(&mut dashboard, &[Menu]);
    assert!(dashboard.guide_line().ends_with("(dm)"));
    press(&mut dashboard, &[B]);
    // A opens the conversation in this Flyout's place; B comes back to the list on that DM.
    press(&mut dashboard, &[Down, A]);
    assert_eq!(asks(&mut dashboard), [DirectAsk::Open("d2".into())]);
    assert_eq!(line(&dashboard), "Conversation loading");
    press(&mut dashboard, &[B]);
    assert_eq!(asks(&mut dashboard), [DirectAsk::Close]);
    assert_eq!(line(&dashboard), "DirectMessages DM(1) of 7");
    // B again: the sixth circle.
    press(&mut dashboard, &[B]);
    assert_eq!(on(&dashboard), Some(Item::AllDMs));
    assert!(!dashboard.guide().direct_open());
}

// ------------------------------------------------------------------------------------------------ writing

fn kind_text(dashboard: &mut Dashboard, text: &str) {
    assert!(dashboard.typed(Typed::Text(text)), "text entry took it");
}

#[test]
fn a_on_the_reply_box_opens_text_entry_which_logs_its_length_and_never_its_text() {
    let stage = in_a_conversation(with_ana());
    let mut dashboard = stage.dashboard;
    press(&mut dashboard, &[A]);
    assert!(
        dashboard.focus_line().starts_with("focus: TextEntry: ") || dashboard.focus_line().starts_with("TextEntry: "),
        "{}",
        dashboard.focus_line()
    );
    assert!(dashboard.focus_line().ends_with("0 chars, upper letters"), "{}", dashboard.focus_line());
    kind_text(&mut dashboard, "Hello there");
    assert!(
        dashboard.focus_line().ends_with("11 chars, upper letters") || dashboard.focus_line().contains("11 chars"),
        "{}",
        dashboard.focus_line()
    );
    assert!(!dashboard.focus_line().contains("Hello"));
    // Each change is the draft at once.
    let draft = |dashboard: &Dashboard| dashboard.guide().dms().draft(&"d1".into()).map(|draft| draft.text.clone());
    assert_eq!(draft(&dashboard).as_deref(), Some("Hello there"));
    // B keeps the draft as typed: back on the reply box, which says so.
    press(&mut dashboard, &[B]);
    assert_eq!(draft(&dashboard).as_deref(), Some("Hello there"));
    assert_eq!(line(&dashboard), "Conversation ReplyBox; draft 11 chars");
    assert!(asks(&mut dashboard).is_empty());
    // Drafts are kept per conversation, in memory, across closing the conversation and the Guide.
    press(&mut dashboard, &[B]);
    dashboard.press(Press { button: Guide, repeat: false });
    assert_eq!(draft(&dashboard).as_deref(), Some("Hello there"));
}

#[test]
fn menu_sends_the_draft_which_shows_at_once_as_sending_and_is_matched_or_fails_after_fifteen_seconds() {
    let mut dashboard = in_a_conversation(with_ana()).dashboard;
    press(&mut dashboard, &[A]);
    kind_text(&mut dashboard, "on my way");
    press(&mut dashboard, &[Menu]);
    let sent = asks(&mut dashboard);
    let [DirectAsk::Send { channel, text, reply: None, nonce }] = &sent[..] else { panic!("{sent:?}") };
    assert_eq!((channel.as_str(), text.as_str()), ("d1", "on my way"));
    // Text entry went, the draft is emptied, focus is on the reply box; the message is there, Sending.
    assert!(dashboard.guide().dms().draft(&"d1".into()).is_none());
    assert_eq!(line(&dashboard), "Conversation ReplyBox; draft 0 chars");
    press(&mut dashboard, &[Up]);
    assert_eq!(line(&dashboard), "Conversation Message(0) of 8, own, sending");
    // The session reports it by its nonce: the local one is replaced.
    let direct = dashboard.direct().clone();
    let mut reported = with_ana();
    let mut message = said(&nonce.to_string(), true, "me", NOW, "on my way");
    message.delivery = Delivery::Sending;
    reported.messages.push(message);
    dashboard.set_direct(&open_with(&direct, reported.clone()));
    assert_eq!(dashboard.guide().dms().outs.len(), 0);
    assert_eq!(line(&dashboard), "Conversation Message(0) of 8, own, sending");
    // Or, as the data source will, under the real id once Discord has it: matched by what it says.
    let mut dashboard = in_a_conversation(with_ana()).dashboard;
    press(&mut dashboard, &[A]);
    kind_text(&mut dashboard, "on my way");
    press(&mut dashboard, &[Menu]);
    asks(&mut dashboard);
    press(&mut dashboard, &[Up]);
    let mut done = with_ana();
    done.messages.push(said("9001", true, "me", NOW + 1000, "on my way"));
    dashboard.set_direct(&open_with(&direct, done));
    assert_eq!(dashboard.guide().dms().outs.len(), 0);
    // Focus followed the message to its real id.
    assert_eq!(line(&dashboard), "Conversation Message(0) of 8, own");
    // Not seen within 15 s: Not sent.
    let mut dashboard = in_a_conversation(with_ana()).dashboard;
    press(&mut dashboard, &[A]);
    kind_text(&mut dashboard, "lost");
    press(&mut dashboard, &[Menu]);
    asks(&mut dashboard);
    dashboard.advance(14.0);
    press(&mut dashboard, &[Up]);
    assert!(line(&dashboard).ends_with("sending"));
    dashboard.advance(1.5);
    assert!(line(&dashboard).ends_with("not sent"), "{}", line(&dashboard));
    // Its menu has Retry and Delete: Retry sends it again as a new one, the old one dropped.
    press(&mut dashboard, &[A, Down, Down]);
    press(&mut dashboard, &[A]);
    let again = asks(&mut dashboard);
    assert!(matches!(&again[0], DirectAsk::Forget { .. }), "{again:?}");
    assert!(matches!(&again[1], DirectAsk::Send { text, nonce: 2, .. } if text.as_str() == "lost"), "{again:?}");
    assert_eq!(dashboard.guide().dms().outs.len(), 1);
    // Delete drops it from the view.
    dashboard.advance(16.0);
    press(&mut dashboard, &[Up, A, Down, Down, Down, A]);
    let gone = asks(&mut dashboard);
    assert!(matches!(&gone[..], [DirectAsk::Forget { .. }]), "{gone:?}");
    assert!(dashboard.guide().dms().outs.is_empty());
}

#[test]
fn blank_text_sends_nothing_and_empties_the_draft() {
    let mut dashboard = in_a_conversation(with_ana()).dashboard;
    press(&mut dashboard, &[A]);
    kind_text(&mut dashboard, "   ");
    press(&mut dashboard, &[Menu]);
    assert!(asks(&mut dashboard).is_empty());
    assert!(dashboard.guide().dms().draft(&"d1".into()).is_none());
}

#[test]
fn x_replies_to_a_message_and_the_reply_goes_with_the_send_and_x_on_the_reply_box_clears_it() {
    let mut dashboard = in_a_conversation(with_ana()).dashboard;
    press(&mut dashboard, &[Up, Up, Up, Up, X]);
    // The keyboard is up at once, the reply set.
    assert!(dashboard.focus_line().contains("TextEntry"), "{}", dashboard.focus_line());
    kind_text(&mut dashboard, "yes");
    press(&mut dashboard, &[B]);
    assert_eq!(line(&dashboard), "Conversation ReplyBox; draft 3 chars, replying");
    press(&mut dashboard, &[X]);
    assert_eq!(line(&dashboard), "Conversation ReplyBox; draft 3 chars");
    press(&mut dashboard, &[Up, Up, Up, Up, X]);
    press(&mut dashboard, &[Menu]);
    let sent = asks(&mut dashboard);
    let [DirectAsk::Send { reply: Some(to), text, .. }] = &sent[..] else { panic!("{sent:?}") };
    assert_eq!((to.as_str(), text.as_str()), ("m5", "yes"));
}

#[test]
fn a_messages_menu_has_reply_add_reaction_and_cancel_and_the_six_reactions_ask_the_page() {
    let mut dashboard = in_a_conversation(with_ana()).dashboard;
    // Message(4) is m3, which has the user's thumbs up.
    press(&mut dashboard, &[Up, Up, Up, Up, Up, A]);
    assert!(dashboard.guide_line().ends_with("(message)"), "{}", dashboard.guide_line());
    press(&mut dashboard, &[Down, A]);
    assert!(dashboard.guide_line().ends_with("(reaction)"), "{}", dashboard.guide_line());
    // B goes back to the message's menu.
    press(&mut dashboard, &[B]);
    assert!(dashboard.guide_line().ends_with("(message)"));
    // Y goes straight to the panel; the first is the user's own thumbs up, which reads Remove and takes it off.
    press(&mut dashboard, &[B, Y]);
    assert!(dashboard.guide_line().ends_with("(reaction)"));
    press(&mut dashboard, &[A]);
    assert_eq!(
        asks(&mut dashboard),
        [DirectAsk::React { channel: "d1".into(), message: "m3".into(), emoji: "\u{1F44D}".into(), on: false }]
    );
    // Fire adds.
    press(&mut dashboard, &[Y, Down, Down, Down, Down, Down, A]);
    assert_eq!(
        asks(&mut dashboard),
        [DirectAsk::React { channel: "d1".into(), message: "m3".into(), emoji: "\u{1F525}".into(), on: true }]
    );
    // Nothing is shown done until the page says so.
    assert_eq!(dashboard.guide().dms().messages(&"d1".into()).len(), 8);
}

#[test]
fn a_captcha_and_a_refusal_are_prompts_over_the_guide() {
    let mut dashboard = in_a_conversation(with_ana()).dashboard;
    dashboard.direct_event(&DirectEvent::Captcha);
    assert_eq!(
        dashboard.guide_line().split(", dialog ").nth(1),
        Some("Menu { prompt: true, row: 0 } \"Discord wants to check it's you\"")
    );
    press(&mut dashboard, &[A]);
    assert!(dashboard.guide().dialog().is_none());
    dashboard.direct_event(&DirectEvent::Refused("You are being rate limited.".into()));
    assert!(dashboard.guide_line().contains("\"Discord didn't send it\""), "{}", dashboard.guide_line());
    assert!(
        !dashboard.guide_line().contains("rate limited"),
        "Discord's reason is the prompt's message, not the log's"
    );
    press(&mut dashboard, &[B]);
    assert!(dashboard.guide().dialog().is_none());
}

// ------------------------------------------------------------------------------------------------ privacy

#[test]
fn no_name_id_or_word_of_a_message_reaches_a_log_line_on_a_walk_of_every_screen() {
    let words = ["Zephyrine", "xylophone", "Quillon", "pelican", "d1-secret-channel", "secret-nonce"];
    let mut dms = vec![
        dm("d1-secret-channel", "Zephyrine", 2, Some(("xylophone at dawn", NOW - 5 * MINUTE))),
        group("g2", "Quillon crew", 3, 1, Some(("pelican", NOW - DAY))),
        dm("d3", "Ana", 0, None),
    ];
    dms[0].picture = Some(picture([0x80, 0x40, 0x40]));
    let direct = ready(dms);
    let conversation = Conversation {
        id: "d1-secret-channel".into(),
        name: "Zephyrine".into(),
        group: false,
        picture: None,
        messages: vec![
            said("m1", false, "Zephyrine", NOW - 10 * MINUTE, "xylophone at dawn, pelican"),
            said("m2", true, "me", NOW - 9 * MINUTE, "Quillon says so"),
        ],
        loading_older: false,
        at_start: true,
        unread_from: Some("m1".into()),
    };
    let mut dashboard = direct_only(&direct);
    let mut lines = vec![dashboard.guide_line(), dashboard.focus_line()];
    let record = |dashboard: &Dashboard, lines: &mut Vec<String>| {
        lines.push(dashboard.guide_line());
        lines.push(dashboard.focus_line());
    };
    for buttons in [&[Right][..], &[Right], &[Right], &[Left, Left, Left], &[Menu], &[B], &[A]] {
        press(&mut dashboard, buttons);
        record(&dashboard, &mut lines);
    }
    dashboard.set_direct(&open_with(&direct, conversation));
    record(&dashboard, &mut lines);
    for buttons in [&[Up][..], &[Up], &[A], &[Menu], &[B], &[Y], &[B], &[A]] {
        press(&mut dashboard, buttons);
        record(&dashboard, &mut lines);
    }
    kind_text(&mut dashboard, "xylophone secret pelican");
    record(&dashboard, &mut lines);
    press(&mut dashboard, &[Menu]);
    record(&dashboard, &mut lines);
    let asked_of_the_session: Vec<String> = asks(&mut dashboard).iter().map(|ask| format!("{ask:?}")).collect();
    dashboard.direct_event(&DirectEvent::Refused("Quillon is not allowed".into()));
    record(&dashboard, &mut lines);
    press(&mut dashboard, &[B, B, B]);
    record(&dashboard, &mut lines);
    press(&mut dashboard, &[Right, Right, Right, Right, A]);
    record(&dashboard, &mut lines);
    // Debug of what is asked of the session, and of the whole Direct, says lengths only.
    let all = format!("{lines:?} {asked_of_the_session:?} {:?}", dashboard.direct());
    for word in words {
        assert!(!all.contains(word), "{word} is in a log line: {all}");
    }
    assert!(lines.iter().any(|line| line.contains("flyout Conversation")), "the walk reached the conversation");
    assert!(lines.iter().any(|line| line.contains("TextEntry")), "and text entry");
}

// ------------------------------------------------------------------------------------------------ pictures of it all

fn staged(discord: &Discord, direct: &Direct) -> Stage {
    Stage::new(on_tab(discord, direct))
}

#[test]
fn shot_tab_focus_on_a_dm_with_unread() {
    staged(&voice_idle(), &ready(seven())).check("direct-tab-dm");
}

#[test]
fn shot_tab_focus_on_the_sixth_circle() {
    let mut stage = staged(&voice_idle(), &ready(seven()));
    press(&mut stage.dashboard, &[Right, Right, Right, Right, Right]);
    stage.check("direct-tab-sixth-circle");
}

#[test]
fn shot_tab_with_a_ring_answer() {
    let mut direct = ready(seven());
    direct.rings = vec![Ring { channel: "d2".into(), caller: "Bo".into(), picture: direct.dms[1].picture.clone() }];
    staged(&Discord::default(), &direct).check("direct-tab-answer");
}

#[test]
fn shot_tab_in_a_call() {
    let mut voice = voice_in_a_call();
    voice.shortcuts = vec![Short {
        id: "600".into(),
        name: "Late night".into(),
        server: Some("The Lads".into()),
        picture: None,
        dm: false,
    }];
    staged(&voice, &ready(seven())).check("direct-tab-in-a-call");
}

#[test]
fn shot_tab_few_dms() {
    let mut stage = staged(&Discord::default(), &ready(seven()[..3].to_vec()));
    press(&mut stage.dashboard, &[Right]);
    stage.check("direct-tab-few");
}

#[test]
fn shot_the_section_states() {
    for (direct, name) in [
        (state(DirectState::Loading), "direct-tab-loading"),
        (state(DirectState::SignedOut), "direct-tab-signed-out"),
        (state(DirectState::NoRuntime), "direct-tab-no-runtime"),
        (state(DirectState::Unreachable), "direct-tab-unreachable"),
        (state(DirectState::PageChanged), "direct-tab-page-changed"),
        (ready(Vec::new()), "direct-tab-empty"),
    ] {
        let mut stage = staged(&Discord::default(), &direct);
        if direct.state == DirectState::Loading {
            // The spinner comes after a second, never for a quick start.
            stage.check("direct-tab-loading-at-once");
            stage.dashboard.advance(1.2);
        }
        stage.check(name);
    }
}

#[test]
fn shot_all_direct_messages() {
    let mut dms = seven();
    dms.insert(
        1,
        dm("d9", "Hal", 12, Some(("see you at the thing tomorrow, bring the charger please", NOW - 2 * DAY))),
    );
    let mut stage = staged(&Discord::default(), &ready(dms));
    press(&mut stage.dashboard, &[Right, Right, Right, Right, Right, A, Up, Up, Up, Up]);
    stage.check("direct-all");
}

#[test]
fn shot_all_direct_messages_group_focused() {
    let mut stage = staged(&Discord::default(), &ready(seven()));
    press(&mut stage.dashboard, &[Right, Right, Right, Right, Right, A, Up, Up]);
    stage.check("direct-all-group");
}

#[test]
fn shot_a_conversation() {
    let mut stage = in_a_conversation(with_ana());
    press(&mut stage.dashboard, &[Up, Up, Up]);
    stage.check("direct-chat");
}

#[test]
fn shot_a_conversation_at_its_foot() {
    let mut stage = in_a_conversation(with_ana());
    stage.check("direct-chat-foot");
}

#[test]
fn shot_a_conversation_with_reactions_and_a_system_line() {
    let mut stage = in_a_conversation(with_ana());
    press(&mut stage.dashboard, &[Up, Up, Up, Up, Up]);
    stage.check("direct-chat-reactions");
}

/// A Group DM: three people, names over their runs, avatars.
fn group_chat() -> Conversation {
    let mut messages = vec![
        said("g1", false, "Cy", NOW - 50 * MINUTE, "who is in for tonight?"),
        said("g2", false, "Cy", NOW - 49 * MINUTE, "starting at ten"),
        said("g3", false, "Di", NOW - 44 * MINUTE, "me, if the lag is better than last time"),
        said("g4", true, "me", NOW - 40 * MINUTE, "in. bringing Bo"),
        with(said("g5", false, "Cy", NOW - 12 * MINUTE, "great"), |_, _, _, reactions| {
            reactions.push(reaction("\u{1F525}", 3, false));
        }),
    ];
    messages[0].author_picture = Some(picture([0x30, 0x90, 0x70]));
    messages[2].author_picture = Some(picture([0x90, 0x60, 0x20]));
    messages[1].author_picture = messages[0].author_picture.clone();
    messages[4].author_picture = messages[0].author_picture.clone();
    Conversation {
        id: "d3".into(),
        name: "Raid night".into(),
        group: true,
        picture: None,
        messages,
        loading_older: false,
        at_start: false,
        unread_from: None,
    }
}

fn open_group() -> Stage {
    let direct = ready(seven());
    let mut dashboard = direct_only(&direct);
    press(&mut dashboard, &[Right, Right, A]);
    dashboard.set_direct(&open_with(&direct, group_chat()));
    dashboard.take_effects();
    let mut stage = Stage::new(dashboard);
    stage.settle();
    stage
}

#[test]
fn shot_a_group_conversation() {
    let mut stage = open_group();
    press(&mut stage.dashboard, &[Up, Up]);
    stage.check("direct-chat-group");
}

#[test]
fn shot_sending_and_not_sent() {
    let mut conversation = with_ana();
    let mut sending = said("n1", true, "me", NOW - 2 * MINUTE, "on my way now");
    sending.delivery = Delivery::Sending;
    let mut failed = said("n2", true, "me", NOW - MINUTE, "this one did not go");
    failed.delivery = Delivery::Failed;
    conversation.messages.extend([sending, failed]);
    let mut stage = in_a_conversation(conversation);
    press(&mut stage.dashboard, &[Up]);
    stage.check("direct-chat-sending");
}

#[test]
fn shot_the_reply_box_with_a_draft_and_replying() {
    let mut stage = in_a_conversation(with_ana());
    press(&mut stage.dashboard, &[A]);
    kind_text(&mut stage.dashboard, "see you in a bit");
    press(&mut stage.dashboard, &[B]);
    stage.check("direct-reply-draft");
    press(&mut stage.dashboard, &[Up, Up, Up, Up, X]);
    kind_text(
        &mut stage.dashboard,
        " and a very long draft that goes on and on past the end of the box so that it is cut",
    );
    press(&mut stage.dashboard, &[B]);
    stage.check("direct-reply-replying");
}

#[test]
fn shot_text_entry_over_the_conversation() {
    let mut stage = in_a_conversation(with_ana());
    press(&mut stage.dashboard, &[Up, Up, Up, X]);
    kind_text(&mut stage.dashboard, "nice, where is that");
    stage.check("direct-writing");
}

#[test]
fn shot_the_dms_menu() {
    let mut stage = staged(&voice_idle(), &ready(seven()));
    press(&mut stage.dashboard, &[Menu]);
    stage.check("direct-dm-menu");
}

#[test]
fn shot_the_messages_menu_and_reactions() {
    let mut stage = in_a_conversation(with_ana());
    press(&mut stage.dashboard, &[Up, Up, Up, Up, A]);
    stage.check("direct-message-menu");
    press(&mut stage.dashboard, &[B, Y]);
    stage.check("direct-reactions");
}

#[test]
fn shot_the_captcha_prompt() {
    let mut stage = in_a_conversation(with_ana());
    stage.dashboard.direct_event(&DirectEvent::Captcha);
    stage.check("direct-captcha");
}
