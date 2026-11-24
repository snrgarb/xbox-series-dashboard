//! Discord's Toasts (docs/spec/party-chats.md, section 7): a Direct Message, a mention and an incoming call, raised
//! from fake events, what the tap of the Guide button opens, the settings that keep them away, and the pictures.
//! Every name is made up. `SHOTS=bless cargo test -p screens --test discord_toasts` rewrites the references.
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use model::discord_dms::{Direct, DirectAsk, DirectEvent, DirectState, Dm, Id, Picture, Ring};
use render::gpu::{self, Gpu};
use render::offscreen::FORMAT;
use screens::guide::{DiscordItem as Item, Focus, Kind, Tab};
use screens::toasts::ToastKind;
use screens::{Dashboard, Effect, Options, Sound, WhilePlaying};
use ui::host::Offscreen;
use ui::{Button, Host, Press};

const NOTIFICATION: Effect = Effect::Sound(Sound::Notification);

fn picture(rgb: [u8; 3]) -> Picture {
    let image = image::RgbImage::from_fn(64, 64, |x, y| {
        let (dx, dy) = (x as f32 - 31.5, y as f32 - 31.5);
        if dx * dx + dy * dy < 14.0 * 14.0 {
            image::Rgb(rgb.map(|c| c.saturating_add(90)))
        } else {
            image::Rgb(rgb)
        }
    });
    let mut bytes = Vec::new();
    image.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
    Picture(Arc::from(bytes))
}

fn dm(id: &str, name: &str, group: bool) -> Dm {
    Dm { id: id.into(), name: name.into(), group, members: 2, picture: None, unread: 1, last: None }
}

fn direct() -> Direct {
    Direct {
        state: DirectState::Ready,
        dms: vec![dm("d1", "Ana", false), dm("d2", "Raid night", true)],
        ..Direct::default()
    }
}

fn dashboard() -> Dashboard {
    let mut dashboard = Dashboard::new(screens::fake_library());
    dashboard.set_direct(&direct());
    dashboard
}

fn message(channel: &str, name: &str, group: Option<&str>, line: &str) -> DirectEvent {
    DirectEvent::Message {
        channel: channel.into(),
        name: name.into(),
        group: group.map(Into::into),
        line: line.into(),
        picture: None,
    }
}

fn ring(channel: &str, caller: &str) -> Ring {
    Ring { channel: channel.into(), caller: caller.into(), picture: None }
}

fn showing(dashboard: &Dashboard) -> Option<(ToastKind, String)> {
    dashboard.toasts().showing().map(|toast| (toast.kind, toast.text.clone()))
}

fn press(dashboard: &mut Dashboard, button: Button) {
    dashboard.press(Press { button, repeat: false });
}

/// A turn of a loop that puts each frame on the screen at once.
fn pass(dashboard: &mut Dashboard, seconds: f32) {
    dashboard.toasts_presented(0.0);
    dashboard.advance(seconds);
    dashboard.toasts_presented(0.0);
}

/// Lets the Toast showing go and the next one come.
fn next(dashboard: &mut Dashboard) {
    pass(dashboard, 6.0);
    pass(dashboard, 1.0);
    pass(dashboard, 1.0);
}

fn with_options(dashboard: &mut Dashboard, change: impl FnOnce(&mut Options)) {
    let mut options = dashboard.options();
    change(&mut options);
    dashboard.set_options(options);
}

fn playing(dashboard: &mut Dashboard) {
    dashboard.launch_started(0);
    dashboard.game_booted();
}

fn item(dashboard: &Dashboard) -> Option<Item> {
    let Focus::Entry(at) = dashboard.guide().focus().1 else { return None };
    match dashboard.guide().entries().get(at)?.kind {
        Kind::Discord(item) => Some(item),
        _ => None,
    }
}

fn asks(dashboard: &mut Dashboard) -> Vec<DirectAsk> {
    dashboard
        .take_effects()
        .into_iter()
        .filter_map(|effect| match effect {
            Effect::Direct(ask) => Some(ask),
            _ => None,
        })
        .collect()
}

// ------------------------------------------------------------------------------------------------ a message

#[test]
fn a_message_raises_a_toast_with_its_first_line_and_the_notification_sound() {
    let mut dashboard = dashboard();
    dashboard.take_effects();
    dashboard.direct_event(&message("d1", "Ana", None, "are you on tonight?\nsecond line"));
    assert_eq!(dashboard.take_effects(), [NOTIFICATION]);
    assert_eq!(showing(&dashboard), Some((ToastKind::DiscordDm, "Ana: are you on tonight?".into())));
    assert!(dashboard.toasts().showing().unwrap().prompt);
    // Not in the Notifications Flyout's list (decision 12).
    assert!(dashboard.notifications().is_empty());
}

#[test]
fn a_group_dm_names_the_group() {
    let mut dashboard = dashboard();
    dashboard.direct_event(&message("d2", "Cy", Some("Raid night"), "bring snacks"));
    assert_eq!(showing(&dashboard).unwrap().1, "Cy in Raid night: bring snacks");
}

#[test]
fn without_text_or_with_show_what_they_say_off_the_line_says_only_who() {
    let mut dashboard = dashboard();
    dashboard.direct_event(&message("d1", "Ana", None, ""));
    assert_eq!(showing(&dashboard).unwrap().1, "Ana sent you a message");
    dashboard.direct_event(&message("d2", "Cy", Some("Raid night"), ""));
    next(&mut dashboard);
    assert_eq!(showing(&dashboard).unwrap().1, "Cy in Raid night");

    let mut dashboard = self::dashboard();
    with_options(&mut dashboard, |options| options.notify_text = false);
    dashboard.direct_event(&message("d1", "Ana", None, "secret words"));
    assert_eq!(showing(&dashboard).unwrap().1, "Ana sent you a message");
}

#[test]
fn a_burst_for_one_conversation_is_one_waiting_toast() {
    let mut dashboard = dashboard();
    dashboard.direct_event(&message("d2", "Cy", Some("Raid night"), "first"));
    // One is showing; the rest wait. Ana's three become one.
    dashboard.direct_event(&message("d1", "Ana", None, "hi"));
    assert_eq!(dashboard.toasts().waiting(), 1);
    dashboard.direct_event(&message("d1", "Ana", None, "are you there"));
    dashboard.direct_event(&message("d1", "Ana", None, "hello?"));
    assert_eq!(dashboard.toasts().waiting(), 1);
    next(&mut dashboard);
    assert_eq!(showing(&dashboard).unwrap().1, "Ana: 3 new messages");
    assert_eq!(dashboard.toasts().waiting(), 0);
}

#[test]
fn none_while_that_conversation_is_open_in_the_guide() {
    let mut dashboard = dashboard();
    press(&mut dashboard, Button::Guide);
    press(&mut dashboard, Button::RightBumper);
    press(&mut dashboard, Button::A);
    assert_eq!(asks(&mut dashboard), [DirectAsk::Open("d1".into())]);
    dashboard.direct_event(&message("d1", "Ana", None, "hi"));
    assert!(dashboard.toasts().on_screen().is_none());
    // Another conversation's message still shows, over the Guide.
    dashboard.direct_event(&message("d2", "Cy", Some("Raid night"), "hi"));
    assert!(showing(&dashboard).is_some());
}

#[test]
fn each_kind_has_its_own_switch() {
    let mut dashboard = dashboard();
    with_options(&mut dashboard, |options| options.notify_messages = false);
    dashboard.direct_event(&message("d1", "Ana", None, "hi"));
    assert!(dashboard.toasts().on_screen().is_none());
    dashboard.direct_event(&DirectEvent::Mention { name: "Bo".into(), channel_name: "general".into() });
    dashboard.direct_event(&DirectEvent::Ring(ring("d1", "Ana")));
    assert_eq!(showing(&dashboard).map(|shown| shown.0), Some(ToastKind::DiscordMention));

    let mut dashboard = self::dashboard();
    with_options(&mut dashboard, |options| options.notify_mentions = false);
    dashboard.direct_event(&DirectEvent::Mention { name: "Bo".into(), channel_name: "general".into() });
    assert!(dashboard.toasts().on_screen().is_none());

    let mut dashboard = self::dashboard();
    with_options(&mut dashboard, |options| options.notify_calls = false);
    dashboard.direct_event(&DirectEvent::Ring(ring("d1", "Ana")));
    assert!(dashboard.toasts().on_screen().is_none());
}

// ------------------------------------------------------------------------------------------------ a mention

#[test]
fn a_mention_names_who_and_where() {
    let mut dashboard = dashboard();
    dashboard.direct_event(&DirectEvent::Mention { name: "Bo".into(), channel_name: "general".into() });
    assert_eq!(showing(&dashboard), Some((ToastKind::DiscordMention, "Bo mentioned you in #general".into())));
    // A tap opens the Guide on the Discord tab (no channel view until #203).
    press(&mut dashboard, Button::Guide);
    assert_eq!(dashboard.guide().focus().0, Tab::Discord);
    assert!(dashboard.guide().dms().chat().is_none());
}

// ------------------------------------------------------------------------------------------------ a call

fn ringing(channel: &str, caller: &str) -> Dashboard {
    let mut dashboard = dashboard();
    let mut direct = direct();
    direct.rings = vec![ring(channel, caller)];
    dashboard.set_direct(&direct);
    dashboard
}

#[test]
fn a_ring_shows_for_30_seconds_before_those_waiting() {
    let mut dashboard = dashboard();
    dashboard.notify("one");
    dashboard.notify("two");
    dashboard.direct_event(&DirectEvent::Ring(ring("d1", "Ana")));
    assert_eq!(dashboard.toasts().waiting(), 2);
    next(&mut dashboard);
    // "one" showed first; the ring cut in before "two".
    assert_eq!(showing(&dashboard), Some((ToastKind::DiscordCall, "Ana is calling".into())));
    pass(&mut dashboard, 28.0);
    assert!(showing(&dashboard).is_some(), "still ringing at 29 s");
    pass(&mut dashboard, 2.5);
    assert!(showing(&dashboard).is_none(), "gone after 30 s");
}

fn call_started(channel: &str, name: &str, group: Option<&str>) -> DirectEvent {
    DirectEvent::CallStarted { channel: channel.into(), name: name.into(), group: group.map(Into::into), picture: None }
}

#[test]
fn a_call_started_says_so_never_sent_you_a_message() {
    let mut dashboard = dashboard();
    dashboard.direct_event(&call_started("d1", "Ana", None));
    assert_eq!(showing(&dashboard), Some((ToastKind::DiscordDm, "Ana started a call".into())));
    let mut dashboard = self::dashboard();
    dashboard.direct_event(&call_started("d2", "Ana", Some("Raid night")));
    assert_eq!(showing(&dashboard), Some((ToastKind::DiscordDm, "Ana started a call in Raid night".into())));
}

#[test]
fn the_ring_takes_the_place_of_its_call_started_and_none_comes_while_it_rings() {
    let mut dashboard = dashboard();
    dashboard.notify("one");
    dashboard.direct_event(&call_started("d1", "Ana", None));
    dashboard.direct_event(&DirectEvent::Ring(ring("d1", "Ana")));
    let waiting: Vec<_> = dashboard.toasts().waiting_kinds();
    assert_eq!(waiting, [ToastKind::DiscordCall], "the call started gave way to the ring");
    dashboard.direct_event(&call_started("d1", "Ana", None));
    assert_eq!(dashboard.toasts().waiting(), 1, "none while it rings");
}

#[test]
fn a_ring_that_ends_takes_its_toast_away_showing_or_waiting() {
    let mut dashboard = dashboard();
    dashboard.direct_event(&DirectEvent::Ring(ring("d1", "Ana")));
    dashboard.toasts_presented(0.0);
    assert!(showing(&dashboard).is_some());
    dashboard.direct_event(&DirectEvent::RingEnded("d1".into()));
    assert!(showing(&dashboard).is_none());

    let mut dashboard = self::dashboard();
    dashboard.notify("one");
    dashboard.direct_event(&DirectEvent::Ring(ring("d1", "Ana")));
    assert_eq!(dashboard.toasts().waiting(), 1);
    dashboard.direct_event(&DirectEvent::RingEnded("d1".into()));
    assert_eq!(dashboard.toasts().waiting(), 0);
}

#[test]
fn one_ring_is_one_toast_by_its_channel() {
    let mut dashboard = dashboard();
    dashboard.notify("one");
    dashboard.direct_event(&DirectEvent::Ring(ring("d1", "Ana")));
    dashboard.direct_event(&DirectEvent::Ring(ring("d1", "Ana")));
    assert_eq!(dashboard.toasts().waiting(), 1);
    // Another channel's ring is another.
    dashboard.direct_event(&DirectEvent::Ring(ring("d2", "Cy")));
    assert_eq!(dashboard.toasts().waiting(), 2);
}

#[test]
fn the_tap_of_a_ring_opens_the_tab_on_answer_and_a_answers() {
    let mut dashboard = ringing("d1", "Ana");
    dashboard.direct_event(&DirectEvent::Ring(ring("d1", "Ana")));
    dashboard.take_effects();
    press(&mut dashboard, Button::Guide);
    assert_eq!(dashboard.guide().focus().0, Tab::Discord);
    assert_eq!(item(&dashboard), Some(Item::Answer));
    dashboard.take_effects();
    press(&mut dashboard, Button::A);
    assert_eq!(asks(&mut dashboard), [DirectAsk::Answer("d1".into())]);
}

// ------------------------------------------------------------------------------------------------ the tap

#[test]
fn the_tap_of_a_message_opens_its_conversation_with_focus_on_the_reply_box() {
    let mut dashboard = dashboard();
    dashboard.direct_event(&message("d1", "Ana", None, "hi"));
    dashboard.take_effects();
    press(&mut dashboard, Button::Guide);
    assert!(dashboard.guide_open());
    assert_eq!(dashboard.guide().focus().0, Tab::Discord);
    assert_eq!(asks(&mut dashboard), [DirectAsk::Open(Id::from("d1"))]);
    let chat = dashboard.guide().dms().chat().expect("a conversation is open");
    assert_eq!((chat.id.clone(), chat.focus.clone()), ("d1".into(), screens::guide::Place::Reply));
    // What the Toast told of is in the Guide now.
    assert!(dashboard.toasts().on_screen().is_none());
}

#[test]
fn the_tap_with_no_discord_tab_just_opens_the_guide() {
    let mut dashboard = Dashboard::new(screens::fake_library());
    dashboard.direct_event(&message("d1", "Ana", None, "hi"));
    press(&mut dashboard, Button::Guide);
    assert!(dashboard.guide_open());
    assert_eq!(dashboard.guide().focus().0, Tab::Landing);
}

// ------------------------------------------------------------------------------------------------ in a game

#[test]
fn while_playing_has_three_choices() {
    let events = |dashboard: &mut Dashboard| {
        dashboard.direct_event(&message("d1", "Ana", None, "hi"));
        dashboard.direct_event(&DirectEvent::Mention { name: "Bo".into(), channel_name: "general".into() });
        dashboard.direct_event(&DirectEvent::Ring(ring("d1", "Ana")));
        let on = dashboard.toasts();
        (u32::from(on.showing().is_some()) + on.waiting() as u32, on.showing().map(|toast| toast.kind))
    };
    // No game: everything shows, whatever the choice.
    for choice in WhilePlaying::ALL {
        let mut dashboard = dashboard();
        with_options(&mut dashboard, |options| options.while_playing = choice);
        assert_eq!(events(&mut dashboard).0, 3, "{choice:?} with no game");
    }
    let game = |choice| {
        let mut dashboard = dashboard();
        with_options(&mut dashboard, |options| options.while_playing = choice);
        playing(&mut dashboard);
        events(&mut dashboard)
    };
    assert_eq!(game(WhilePlaying::ShowAll).0, 3);
    assert_eq!(game(WhilePlaying::CallsOnly), (1, Some(ToastKind::DiscordCall)));
    assert_eq!(game(WhilePlaying::None).0, 0);
}

// ------------------------------------------------------------------------------------------------ privacy

#[test]
fn what_is_said_of_these_toasts_is_never_a_name_or_a_word() {
    for kind in [ToastKind::DiscordDm, ToastKind::DiscordMention, ToastKind::DiscordCall] {
        assert!(kind.is_private());
        assert!(["DiscordDm", "DiscordMention", "DiscordCall"].contains(&format!("{kind:?}").as_str()));
    }
    assert!(!ToastKind::Notification.is_private());
    let events = [
        message("d1", "Ana", None, "secret words"),
        DirectEvent::Mention { name: "Bo".into(), channel_name: "general".into() },
        DirectEvent::Ring(ring("d1", "Ana")),
    ];
    for event in &events {
        let said = format!("{event:?}");
        assert!(!said.contains("Ana") && !said.contains("secret") && !said.contains("general"), "{said}");
    }
    // The Guide's own log line after a tap names no one either.
    let mut dashboard = dashboard();
    dashboard.direct_event(&events[0]);
    press(&mut dashboard, Button::Guide);
    let line = dashboard.guide_line();
    assert!(!line.contains("Ana") && !line.contains("secret"), "{line}");
}

// ------------------------------------------------------------------------------------------------ pictures

fn target_dir() -> PathBuf {
    let from_env = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from);
    from_env.unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"))
}

static DRAWING: Mutex<()> = Mutex::new(());

struct Stage {
    gpu: Gpu,
    host: Host,
    target: Offscreen,
    dashboard: Dashboard,
    _drawing: MutexGuard<'static, ()>,
}

impl Stage {
    fn new() -> Self {
        let drawing = DRAWING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let fonts = target_dir().join("fonts");
        assert!(fonts.join("selawk.ttf").is_file(), "no test fonts: run tools/test-fonts.sh");
        std::env::set_var("DASHBOARD_FONTS", fonts);
        let gpu = gpu::init(gpu::instance(None), None);
        let host = Host::new(&gpu, FORMAT, || {});
        let target = Offscreen::new(&gpu, (1920, 1080));
        let mut dashboard = dashboard();
        dashboard.set_clock(Some("9:41 AM".into()));
        Self { gpu, host, target, dashboard, _drawing: drawing }
    }

    fn check(&mut self, name: &str) {
        self.host.store.set_time(10.0);
        let Self { gpu, host, target, dashboard, .. } = self;
        let pixels = target.still(gpu, host, |ui| dashboard.draw_all(ui));
        let image = image::RgbaImage::from_raw(target.size.0, target.size.1, pixels).unwrap();
        let got = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.png"));
        image.save(&got).unwrap();
        shots::check(name, &got, &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference"));
    }
}

fn shot(name: &str, events: &[DirectEvent]) {
    let mut stage = Stage::new();
    for event in events {
        stage.dashboard.direct_event(event);
    }
    stage.check(name);
}

fn with_picture(event: DirectEvent, rgb: [u8; 3]) -> DirectEvent {
    match event {
        DirectEvent::Message { channel, name, group, line, .. } => {
            DirectEvent::Message { channel, name, group, line, picture: Some(picture(rgb)) }
        }
        DirectEvent::Ring(ring) => DirectEvent::Ring(Ring { picture: Some(picture(rgb)), ..ring }),
        other => other,
    }
}

#[test]
#[ignore = "screenshot"]
fn the_toasts_drawn() {
    // A DM with the sender's avatar: round, 80 across, bare on the card.
    shot("toast-discord-dm", &[with_picture(message("d1", "Ana", None, "are you on tonight?"), [0xC0, 0x50, 0x40])]);
    // With no picture: Discord's mark on the lighter square.
    shot("toast-discord-dm-no-picture", &[message("d1", "Ana", None, "are you on tonight?")]);
    shot(
        "toast-discord-group",
        &[with_picture(message("d2", "Cy", Some("Raid night"), "bring snacks"), [0x20, 0x60, 0xA0])],
    );
    shot("toast-discord-sent-you", &[with_picture(message("d1", "Ana", None, ""), [0xC0, 0x50, 0x40])]);
    // A burst: another Toast is showing, three messages wait as one, which then shows.
    let mut stage = Stage::new();
    stage.dashboard.notify("one");
    for text in ["hi", "are you there", "hello?"] {
        stage.dashboard.direct_event(&message("d1", "Ana", None, text));
    }
    next(&mut stage.dashboard);
    stage.check("toast-discord-burst");
    // One device draws at a time: this one is let go before the next is made.
    drop(stage);
    shot("toast-discord-mention", &[DirectEvent::Mention { name: "Bo".into(), channel_name: "general".into() }]);
    shot("toast-discord-call", &[with_picture(DirectEvent::Ring(ring("d1", "Ana")), [0xC0, 0x50, 0x40])]);
    shot("toast-discord-call-no-picture", &[DirectEvent::Ring(ring("d1", "Ana"))]);
}
