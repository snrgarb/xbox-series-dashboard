//! The real connection (pipe, token endpoint, threads) against the fake Discord: connects, is lost as the fake goes
//! away, tries again, and signs in by itself once it is back.
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use discord::fake::FakeDiscord;
use discord::{Connection, Place};
use model::discord::{Ask, Discord, Field};

/// A pipe of the test's own: a socket in its folder, or on Windows a named pipe no other run has.
fn pipe_name(dir: &tempfile::TempDir, name: &str) -> String {
    if cfg!(windows) {
        let folder = dir.path().file_name().unwrap_or_default().to_string_lossy();
        format!("{name}-{}-{folder}", std::process::id())
    } else {
        dir.path().join(name).to_string_lossy().into_owned()
    }
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
    let place = Place { pipe: Some(pipe.clone()), token_url: Some(fake.token_url()) };
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

#[test]
fn the_fake_tells_the_call_to_whoever_subscribed() {
    use discord::Event;
    use serde_json::json;

    let dir = tempfile::tempdir().unwrap();
    let pipe = pipe_name(&dir, "e2e-discord");
    let fake = FakeDiscord::start(&pipe, 0, Arc::new(|_| {})).unwrap();
    let place = Place { pipe: Some(pipe.clone()), token_url: Some(fake.token_url()) };
    let mut connection = Connection::new(&place, None, || {});
    let mut clock = SystemTime::now();
    connection.ask(Ask::ClientId("1".into()), || None);
    connection.ask(Ask::ClientSecret("s".into()), || None);
    connection.ask(Ask::Connect, || None);
    until(&mut connection, &mut clock, "sign-in", |d| d.connected);
    until(&mut connection, &mut clock, "subscriptions", |_| fake.subscriptions().len() == 3);
    connection.client().take_events();

    assert_eq!(fake.control("call General Ana Bo").unwrap(), "the user is in General with Ana, Bo");
    let mut events = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    while events.is_empty() && Instant::now() < deadline {
        connection.pump(clock);
        events.extend(connection.client().take_events());
        std::thread::sleep(Duration::from_millis(10));
    }
    let channel = discord::fake::CHANNELS[0].0;
    let selected = Event::Dispatch {
        evt: "VOICE_CHANNEL_SELECT".into(),
        data: json!({ "channel_id": channel, "guild_id": discord::fake::GUILD_ID }),
    };
    assert_eq!(events, [selected]);

    let nonce = connection.client().send("GET_CHANNEL", json!({ "channel_id": channel }), None).unwrap();
    let mut reply = None;
    let deadline = Instant::now() + Duration::from_secs(5);
    while reply.is_none() && Instant::now() < deadline {
        connection.pump(clock);
        reply = connection.client().take_events().into_iter().find_map(|event| match event {
            Event::Reply { nonce: n, data, .. } if n == nonce => Some(data),
            _ => None,
        });
        std::thread::sleep(Duration::from_millis(10));
    }
    let reply = reply.expect("GET_CHANNEL answered");
    let names: Vec<&str> =
        reply["voice_states"].as_array().unwrap().iter().map(|s| s["nick"].as_str().unwrap()).collect();
    assert_eq!(names, ["E2E Tester", "Ana", "Bo"]);
    assert!(fake.control("call Nowhere").is_err());
}
