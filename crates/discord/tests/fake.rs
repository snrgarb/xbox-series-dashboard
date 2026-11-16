//! The real connection (pipe, token endpoint, threads) against the fake Discord: connects, is lost as the fake goes
//! away, tries again, and signs in by itself once it is back; follows a Call, its people's avatars fetched from the
//! fake's stand-in CDN, and Mute and Leave shown only once the fake has carried them out.
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use discord::fake::{FakeDiscord, CHANNELS, GUILD_ID, GUILD_NAME};
use discord::{Connection, Place};
use model::discord::{Ask, Browse, BrowseAsk, Call, CallAsk, Discord, Field, Person, Voice};

/// A pipe no other run has: on Windows a named pipe, elsewhere a socket of that name in the runtime folder (a
/// socket's path has to be short, which a test's folder in a deep checkout is not).
fn pipe_name(dir: &tempfile::TempDir, name: &str) -> String {
    let folder = dir.path().file_name().unwrap_or_default().to_string_lossy();
    format!("{name}-{}-{folder}", std::process::id())
}

/// Pumps `connection` until `done` holds of what Settings shows, the clock running fast.
fn until(connection: &mut Connection, clock: &mut SystemTime, what: &str, done: impl Fn(&Discord) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        connection.pump(*clock);
        if done(connection.discord()) {
            return;
        }
        assert!(Instant::now() < deadline, "no {what}: {:?}", connection.discord());
        std::thread::sleep(Duration::from_millis(10));
        // Time runs fast so the back-off's seconds pass at once.
        *clock += Duration::from_millis(500);
    }
}

#[test]
fn connects_to_the_fake_is_lost_as_it_goes_and_signs_in_again_as_it_comes_back() {
    let dir = tempfile::tempdir().unwrap();
    let pipe = pipe_name(&dir, "e2e-discord");
    let fake = FakeDiscord::start(&pipe, 0, Arc::new(|line| eprintln!("{line}"))).unwrap();
    let place = Place { pipe: Some(pipe.clone()), token_url: Some(fake.token_url()), cdn_url: None };
    let mut connection = Connection::new(&place, Some(dir.path()), || {});
    connection.start();
    let mut clock = SystemTime::now();

    connection.ask(Ask::Paste(Field::ClientId), || Some("1290000000000000001".into()));
    connection.ask(Ask::Paste(Field::ClientSecret), || Some("a-secret".into()));
    connection.ask(Ask::Paste(Field::ClientSecret), || None);
    assert!(connection.discord().has_secret, "an empty clipboard pastes nothing");
    connection.ask(Ask::Connect, || None);
    until(&mut connection, &mut clock, "sign-in", |d| d.connected);
    assert_eq!(connection.discord().status, "Connected as E2E Tester");
    until(&mut connection, &mut clock, "subscriptions", |_| fake.subscriptions().len() == 3);

    // The secrets are in the data folder, and the secret is not there as it is.
    let file = std::fs::read(dir.path().join(discord::secrets::FILE)).unwrap();
    assert!(!file.is_empty());

    fake.control("go-away").unwrap();
    until(&mut connection, &mut clock, "the loss", |d| !d.connected && d.retrying);
    assert_eq!(connection.discord().status, "Discord isn't running");

    fake.control("come-back").unwrap();
    until(&mut connection, &mut clock, "sign-in again", |d| d.connected);
    assert_eq!(connection.discord().status, "Connected as E2E Tester");

    // The Dashboard starting again signs in with the saved tokens, asking nothing.
    drop(connection);
    let mut connection = Connection::new(&place, Some(dir.path()), || {});
    connection.start();
    until(&mut connection, &mut clock, "sign-in with the saved tokens", |d| d.connected);

    connection.ask(Ask::Disconnect, || None);
    assert_eq!(connection.discord().status, "Not connected");
    assert!(!connection.discord().can_disconnect());
}

/// Signed in to a fake of the test's own, avatars from its stand-in CDN into `dir`.
fn signed_in(dir: &tempfile::TempDir) -> (FakeDiscord, Connection, SystemTime) {
    let pipe = pipe_name(dir, "e2e-discord");
    let fake = FakeDiscord::start(&pipe, 0, Arc::new(|line| eprintln!("{line}"))).unwrap();
    let place = Place { pipe: Some(pipe.clone()), token_url: Some(fake.token_url()), cdn_url: Some(fake.cdn_url()) };
    let mut connection = Connection::new(&place, Some(dir.path()), || {});
    let mut clock = SystemTime::now();
    connection.ask(Ask::ClientId("1".into()), || None);
    connection.ask(Ask::ClientSecret("s".into()), || None);
    connection.ask(Ask::Connect, || None);
    until(&mut connection, &mut clock, "sign-in", |d| d.connected);
    until(&mut connection, &mut clock, "subscriptions", |_| fake.subscriptions().len() == 3);
    (fake, connection, clock)
}

fn call(discord: &Discord) -> Option<&Call> {
    discord.voice.call.as_ref()
}

/// Who is in the Call as the view has them: each person's name, with `*` while speaking, `m` muted, `d` deafened.
fn people(discord: &Discord) -> Vec<String> {
    let Some(call) = call(discord) else { return Vec::new() };
    let mark = |p: &Person| {
        let marks: String = [(p.speaking, '*'), (p.muted, 'm'), (p.deafened, 'd')]
            .iter()
            .filter_map(|&(on, c)| on.then_some(c))
            .collect();
        format!("{}{}", p.name, marks)
    };
    call.people.iter().map(mark).collect()
}

#[test]
fn the_call_follows_the_fake_and_mute_and_leave_show_only_once_it_confirms() {
    let dir = tempfile::tempdir().unwrap();
    let (fake, mut connection, mut clock) = signed_in(&dir);
    assert!(call(connection.discord()).is_none());

    // A Call arrives: the channel, the people in it with the user first, and each one's avatar fetched from the
    // stand-in CDN into the cache.
    fake.control("call General Ana Bo").unwrap();
    until(&mut connection, &mut clock, "the call", |d| people(d) == ["E2E Tester", "Ana", "Bo"]);
    assert_eq!(call(connection.discord()).unwrap().title, "General");
    until(&mut connection, &mut clock, "the avatars", |d| call(d).unwrap().people.iter().all(|p| p.avatar.is_some()));
    for person in &call(connection.discord()).unwrap().people {
        let file = person.avatar.as_ref().unwrap();
        assert!(file.starts_with(dir.path().join("art").join("discord")), "{file:?}");
        assert!(image_size(file) == (64, 64));
    }
    // The user's own is Discord's default one, by their id.
    let me = &call(connection.discord()).unwrap().people[0];
    assert!(me.me && me.avatar.as_ref().unwrap().ends_with("embed/0.png"), "{:?}", me.avatar);

    // Someone joins, speaks, stops; another is muted and deafened by the server.
    fake.control("join Cy").unwrap();
    until(&mut connection, &mut clock, "Cy", |d| people(d) == ["E2E Tester", "Ana", "Bo", "Cy"]);
    fake.control("speak Ana").unwrap();
    until(&mut connection, &mut clock, "Ana's ring", |d| people(d)[1] == "Ana*");
    fake.control("hush Ana").unwrap();
    fake.control("deafen Bo").unwrap();
    until(&mut connection, &mut clock, "Bo deafened", |d| people(d)[1..3] == ["Ana", "Bod"]);
    fake.control("leave Cy").unwrap();
    until(&mut connection, &mut clock, "Cy gone", |d| people(d) == ["E2E Tester", "Ana", "Bod"]);

    // Mute, held by the fake: nothing shows until it confirms.
    fake.control("hold").unwrap();
    connection.ask(Ask::Call(CallAsk::Mute), || None);
    for _ in 0..20 {
        connection.pump(clock);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!connection.discord().voice.mute, "not muted until Discord says so");
    fake.control("confirm").unwrap();
    until(&mut connection, &mut clock, "muted", |d| d.voice.mute && people(d)[0] == "E2E Testerm");

    // Mute again is unmute; refused, the connection stays up and nothing changes.
    fake.control("hold").unwrap();
    connection.ask(Ask::Call(CallAsk::Mute), || None);
    std::thread::sleep(Duration::from_millis(50));
    fake.control("refuse Nope").unwrap();
    for _ in 0..30 {
        connection.pump(clock);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(connection.discord().connected && connection.discord().voice.mute);

    // Leave, held: the Call stays until the fake confirms.
    fake.control("hold").unwrap();
    connection.ask(Ask::Call(CallAsk::Leave), || None);
    for _ in 0..20 {
        connection.pump(clock);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(call(connection.discord()).is_some(), "still in the call until Discord says otherwise");
    fake.control("confirm").unwrap();
    until(&mut connection, &mut clock, "out of the call", |d| call(d).is_none());
    // The channel's own events are no longer followed.
    until(&mut connection, &mut clock, "unsubscribed", |_| fake.subscriptions().len() == 3);

    // Discord going ends the Call it was in, and forgets the voice settings.
    fake.control("call Late_night Ana").unwrap();
    until(&mut connection, &mut clock, "a call again", |d| call(d).is_some_and(|c| c.title == "Late night"));
    fake.control("go-away").unwrap();
    until(&mut connection, &mut clock, "the loss", |d| !d.connected);
    assert_eq!(connection.discord().voice, Voice::default());
    assert!(fake.control("call Nowhere").is_err());
}

#[test]
fn servers_list_from_the_fake_a_channel_joined_becomes_a_shortcut_kept_across_starts_and_rejoined_in_one_ask() {
    let dir = tempfile::tempdir().unwrap();
    let (fake, mut connection, mut clock) = signed_in(&dir);
    let listed = |d: &Discord| (0..d.browse.len()).map(|at| d.browse.name(at).unwrap().to_string()).collect::<Vec<_>>();

    // Servers: never joined, so by name ignoring case, each icon fetched from the stand-in CDN.
    connection.ask(Ask::Browse(BrowseAsk::Servers), || None);
    assert!(connection.discord().browse.loading());
    until(&mut connection, &mut clock, "the servers", |d| !d.browse.loading());
    assert_eq!(listed(connection.discord()), ["Another Server", "art club", "Stand-in Server"]);
    until(&mut connection, &mut clock, "the icons", |d| match &d.browse {
        Browse::Servers { servers, .. } => servers[1].icon.is_some() && servers[2].icon.is_some(),
        _ => false,
    });
    let Browse::Servers { servers, .. } = &connection.discord().browse else { unreachable!() };
    assert!(servers[0].icon.is_none(), "Another Server has none");

    // A server: its voice and stage channels, with who is in each.
    fake.control("in Late_night Ana Bo").unwrap();
    connection.ask(Ask::Browse(BrowseAsk::Server { id: GUILD_ID.into(), name: GUILD_NAME.into() }), || None);
    until(&mut connection, &mut clock, "the channels and who is in them", |d| match &d.browse {
        Browse::Server { loading: false, channels, .. } => {
            channels.len() == 2 && channels.iter().all(|c| c.occupants.is_some())
        }
        _ => false,
    });
    assert_eq!(listed(connection.discord()), ["General", "Late night"]);
    let Browse::Server { channels, .. } = &connection.discord().browse else { unreachable!() };
    assert_eq!(channels[0].occupants.as_ref().map(Vec::len), Some(0));
    assert_eq!(channels[1].occupants.as_ref().map(Vec::len), Some(2));
    assert!(call(connection.discord()).is_none(), "who is in a channel is not the Call");

    // Joined: the Flyout closes, and the Call is the channel once Discord says so.
    connection.ask(Ask::Join(CHANNELS[1].0.into()), || None);
    assert_eq!(connection.discord().browse, Browse::Closed);
    until(&mut connection, &mut clock, "the call", |d| call(d).is_some_and(|c| c.title == "Late night"));
    fake.control("hang-up").unwrap();
    until(&mut connection, &mut clock, "the shortcut", |d| {
        call(d).is_none() && d.shortcuts.first().is_some_and(|s| s.server.as_deref() == Some(GUILD_NAME))
    });
    let shortcut = connection.discord().shortcuts[0].clone();
    assert_eq!((shortcut.name.as_str(), shortcut.dm), ("Late night", false));
    until(&mut connection, &mut clock, "its icon", |d| d.shortcuts[0].picture.is_some());

    // A DM: its Shortcut named after who was in it.
    fake.control("dm Cy").unwrap();
    until(&mut connection, &mut clock, "the DM", |d| call(d).is_some());
    fake.control("hang-up").unwrap();
    until(&mut connection, &mut clock, "two shortcuts", |d| d.shortcuts.len() == 2 && call(d).is_none());
    assert_eq!(connection.discord().shortcuts[0].name, "Cy");
    assert!(connection.discord().shortcuts[0].dm);

    // The Dashboard starting again: the Shortcuts are there before Discord answers anything.
    drop(connection);
    let place = Place {
        pipe: Some(pipe_name(&dir, "e2e-discord")),
        token_url: Some(fake.token_url()),
        cdn_url: Some(fake.cdn_url()),
    };
    let mut connection = Connection::new(&place, Some(dir.path()), || {});
    let names: Vec<String> = connection.discord().shortcuts.iter().map(|s| s.name.clone()).collect();
    assert_eq!(names, ["Cy", "Late night"]);

    // Rejoined in one ask, and removed.
    connection.start();
    until(&mut connection, &mut clock, "sign-in", |d| d.connected);
    connection.ask(Ask::Join(CHANNELS[1].0.into()), || None);
    until(&mut connection, &mut clock, "the call again", |d| call(d).is_some_and(|c| c.title == "Late night"));
    connection.ask(Ask::RemoveShortcut(CHANNELS[1].0.into()), || None);
    let names: Vec<String> = connection.discord().shortcuts.iter().map(|s| s.name.clone()).collect();
    assert_eq!(names, ["Cy"]);
}

fn image_size(file: &std::path::Path) -> (u32, u32) {
    let bytes = std::fs::read(file).unwrap();
    // A PNG's IHDR: its width and height, big-endian, from byte 16.
    let be = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap());
    (be(16), be(20))
}
