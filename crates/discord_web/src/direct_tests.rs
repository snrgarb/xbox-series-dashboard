//! The session's Direct Messages, through a fake web view that lets a test feed the page's messages and read what the
//! session asked the page (made-up names and ids only).
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use model::discord_dms::*;

use crate::tests::{captured, capturing};
use crate::*;

/// A web view that opens and says nothing until the test does. What the session sends it is read as it is sent (the
/// poke the session makes after each command), so a test sees it at once; a Stop or SignOut ends it, as a host does.
#[derive(Clone, Default)]
struct Fake {
    tell: Arc<Mutex<Option<Tell>>>,
    log: Arc<Mutex<Vec<String>>>,
}

impl Backend for Fake {
    fn runtime(&self) -> Result<String, String> {
        Ok("157.0.0.0 (fake)".into())
    }

    fn spawn(&self, _: PathBuf, _: String, commands: Receiver<Command>, tell: Tell) -> Result<Spawned, String> {
        *self.tell.lock().unwrap() = Some(tell.clone());
        let log = self.log.clone();
        let poke = move || {
            while let Ok(command) = commands.try_recv() {
                let (said, ends) = match command {
                    Command::Page(op) => (format!("{op:?}"), false),
                    Command::Stop | Command::SignOut => ("other".to_string(), true),
                    _ => ("other".to_string(), false),
                };
                log.lock().unwrap().push(said);
                if ends {
                    tell(Report::Ended { signed_out: None });
                }
            }
        };
        Ok(Spawned { poke: Box::new(poke), thread: std::thread::spawn(|| {}) })
    }
}

impl Fake {
    fn say(&self, report: Report) {
        (self.tell.lock().unwrap().as_ref().expect("a web view is up"))(report);
    }

    fn page(&self, json: &str) {
        self.say(Report::Page(json.to_string()));
    }

    /// The asks of the page since last looked.
    fn asked(&self) -> Vec<String> {
        std::mem::take(&mut *self.log.lock().unwrap())
    }
}

/// A session with a kept folder (so its web view starts), signed in, its CDN none.
fn signed_in(data: &Path) -> (Session, Fake) {
    std::fs::create_dir_all(folder(data, 7)).unwrap();
    std::fs::write(folder(data, 7).join("Local State"), b"made up").unwrap();
    let fake = Fake::default();
    let mut session = Session::with(Some("http://stand-in.test"), Some(data), || {}, Box::new(fake.clone()));
    session.set_profile(Some(7));
    session.set_on(true);
    fake.say(Report::State(WebState::SignedIn));
    session.pump();
    (session, fake)
}

const OK: &str = r#"{"k":"health","ok":true,"found":{"channels":["ChannelStore","getChannel"],"messages":["MessageStore","getMessages"]},"missing":[],"older":true}"#;

fn dm_json(id: &str, name: &str, unread: u32) -> String {
    format!(
        r#"{{"id":"{id}","name":"{name}","group":false,"members":2,"pic":"avatars/9/hash.png?size=128","unread":{unread},"mentions":0}}"#
    )
}

fn list(dms: &[String]) -> String {
    format!(r#"{{"k":"dms","dms":[{}]}}"#, dms.join(","))
}

const ANA: &str = "700000000000000001";
const BO: &str = "700000000000000002";

fn conv(id: &str, count: usize, more: bool, older: bool) -> String {
    let msgs: Vec<String> = (0..count)
        .map(|i| {
            format!(
                r#"{{"id":"{}","author":{{"id":"55","name":"Ana Example"}},"own":{},"at":{},"text":"secret words {i}"}}"#,
                900 + i,
                i % 2 == 1,
                1_700_000_000_000i64 + i as i64
            )
        })
        .collect();
    format!(
        r#"{{"k":"conv","id":"{id}","loaded":true,"more":{more},"older":{older},"unread_from":"901","msgs":[{}]}}"#,
        msgs.join(",")
    )
}

#[test]
fn the_state_follows_the_web_session() {
    let data = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(folder(data.path(), 7)).unwrap();
    std::fs::write(folder(data.path(), 7).join("Local State"), b"made up").unwrap();
    let fake = Fake::default();
    let mut s = Session::with(Some("http://stand-in.test"), Some(data.path()), || {}, Box::new(fake.clone()));
    s.set_profile(Some(7));
    assert_eq!(s.direct().state, DirectState::Off);
    s.set_on(true);
    assert_eq!((s.web().state, s.direct().state), (WebState::Checking, DirectState::Loading));
    fake.say(Report::State(WebState::SignedIn));
    assert!(s.pump(), "the screens' state changed");
    assert_eq!(s.direct().state, DirectState::Loading, "signed in, the list not yet read");
    fake.page(OK);
    s.pump();
    assert_eq!(s.direct().state, DirectState::Loading, "a health report is not a list");
    fake.page(&list(&[]));
    assert!(s.pump());
    assert_eq!(s.direct().state, DirectState::Ready, "an empty list is Ready");
    fake.page(r#"{"k":"health","ok":false,"found":{},"missing":["readState.getUnreadCount"]}"#);
    s.pump();
    assert_eq!(s.direct().state, DirectState::PageChanged);
    assert!(s.direct().dms.is_empty());
    fake.page(OK);
    s.pump();
    assert_eq!(s.direct().state, DirectState::Ready);
    fake.say(Report::State(WebState::SignedOut));
    s.pump();
    assert_eq!(s.direct().state, DirectState::SignedOut);
    fake.say(Report::State(WebState::Failed));
    s.pump();
    assert_eq!(s.direct().state, DirectState::Unreachable);
    s.set_on(false);
    assert_eq!(s.direct().state, DirectState::Off);
}

#[test]
fn no_runtime_is_no_runtime() {
    struct Missing;
    impl Backend for Missing {
        fn runtime(&self) -> Result<String, String> {
            Err("no runtime (fake)".into())
        }
        fn spawn(&self, _: PathBuf, _: String, _: Receiver<Command>, _: Tell) -> Result<Spawned, String> {
            Err("no runtime (fake)".into())
        }
    }
    let data = tempfile::tempdir().unwrap();
    let mut s = Session::with(None, Some(data.path()), || {}, Box::new(Missing));
    s.set_profile(Some(7));
    s.set_on(true);
    assert_eq!(s.direct().state, DirectState::NoRuntime);
}

#[test]
fn the_list_is_the_models_in_discords_order_and_pump_says_when_it_changed() {
    let data = tempfile::tempdir().unwrap();
    let (mut s, fake) = signed_in(data.path());
    assert!(fake.asked().contains(&"Resend".to_string()), "signed in: the page is asked to send everything");
    let json = r#"{"k":"dms","dms":[
        {"id":"1","name":"Ana Example","members":2,"unread":3,"last":{"at":1700000000000,"text":"hello <@5>","who":"Ana Example","names":{"5":"Bo"}}},
        {"id":"2","name":"The Group","group":true,"members":4,"unread":0,"last":{"at":1700000100000}},
        {"id":"3","name":"Quiet","members":2}]}"#;
    fake.page(json);
    assert!(s.pump());
    let d = s.direct();
    assert_eq!(d.state, DirectState::Ready);
    assert_eq!(d.dms.iter().map(|dm| dm.id.as_str()).collect::<Vec<_>>(), ["1", "2", "3"]);
    assert_eq!(d.dms[0].unread, 3);
    assert_eq!(d.dms[0].last.as_ref().map(|(l, at)| (l.as_str(), *at)), Some(("hello @Bo", 1_700_000_000_000)));
    assert!(d.dms[1].group && d.dms[1].members == 4);
    assert_eq!(d.dms[1].last.as_ref().map(|(l, at)| (l.as_str(), *at)), Some(("", 1_700_000_100_000)), "its time only");
    assert_eq!(d.dms[2].last, None);
    assert_eq!(d.counts(), "dms 3, unread 3 in 1");
    assert!(d.dms.iter().all(|dm| dm.picture.is_none()), "no CDN: no pictures");
    fake.page(json);
    assert!(!s.pump(), "the same list again changes nothing");
}

#[test]
fn a_conversation_opens_by_ask_and_closes_forgetting_its_messages() {
    let data = tempfile::tempdir().unwrap();
    let (mut s, fake) = signed_in(data.path());
    fake.page(OK);
    fake.page(&list(&[dm_json(ANA, "Ana Example", 2)]));
    s.pump();
    fake.asked();
    s.ask_direct(DirectAsk::Open("not digits; alert(1)".into()));
    s.ask_direct(DirectAsk::Open("".into()));
    assert!(fake.asked().is_empty(), "an id that is not digits is never sent to the page");
    s.ask_direct(DirectAsk::Open(ANA.into()));
    assert_eq!(fake.asked(), vec![format!("Open(\"{ANA}\")")]);
    assert!(s.direct().open.is_none(), "not until its messages have come");
    // A conversation the screens did not ask for is not taken.
    fake.page(&conv(BO, 2, false, false));
    s.pump();
    assert!(s.direct().open.is_none());
    fake.page(&conv(ANA, 3, true, false));
    assert!(s.pump());
    let open = s.direct().open.as_ref().unwrap();
    assert_eq!(open.id, Id::from(ANA));
    assert_eq!(open.name.as_str(), "Ana Example");
    assert_eq!(open.messages.len(), 3);
    assert!(!open.at_start && !open.loading_older);
    assert_eq!(open.unread_from, Some(Id::from("901")));
    assert!(open.messages[1].own && !open.messages[0].own);
    assert_eq!(open.messages[0].at, 1_700_000_000_000);
    // Older messages are asked of the page, and shown loading until the page says it is done.
    s.ask_direct(DirectAsk::Older);
    assert_eq!(fake.asked(), vec!["Older".to_string()]);
    fake.page(&conv(ANA, 3, true, true));
    s.pump();
    assert!(s.direct().open.as_ref().unwrap().loading_older);
    fake.page(&conv(ANA, 5, false, false));
    s.pump();
    let open = s.direct().open.as_ref().unwrap();
    assert!(open.at_start && !open.loading_older && open.messages.len() == 5);
    s.ask_direct(DirectAsk::Older);
    assert!(fake.asked().is_empty(), "at the start there is no more to ask for");
    s.ask_direct(DirectAsk::Close);
    assert_eq!(fake.asked(), vec!["Close".to_string()]);
    assert!(s.direct().open.is_none() && s.reader.conv.is_none());
    fake.page(&conv(ANA, 5, false, false));
    s.pump();
    assert!(s.direct().open.is_none(), "closed: its pushes are not taken");
}

#[test]
fn older_does_nothing_where_discords_action_was_not_found() {
    let data = tempfile::tempdir().unwrap();
    let (mut s, fake) = signed_in(data.path());
    capturing();
    fake.page(r#"{"k":"health","ok":true,"found":{"channels":["ChannelStore"]},"missing":[],"older":false}"#);
    fake.page(&list(&[dm_json(ANA, "Ana Example", 0)]));
    s.ask_direct(DirectAsk::Open(ANA.into()));
    fake.page(&conv(ANA, 3, true, false));
    s.pump();
    assert_eq!(s.direct().state, DirectState::Ready, "reading works without it");
    fake.asked();
    s.ask_direct(DirectAsk::Older);
    s.ask_direct(DirectAsk::Older);
    assert!(fake.asked().is_empty());
    let said = captured().iter().filter(|l| l.contains("older messages")).count();
    assert_eq!(said, 1, "said once");
}

#[test]
fn the_asks_not_built_yet_say_so_once_by_kind() {
    let data = tempfile::tempdir().unwrap();
    let (mut s, fake) = signed_in(data.path());
    capturing();
    fake.asked();
    for _ in 0..2 {
        s.ask_direct(DirectAsk::Send { channel: ANA.into(), text: "secret".into(), reply: None, nonce: 1 });
        s.ask_direct(DirectAsk::Forget { channel: ANA.into(), message: "1".into() });
        s.ask_direct(DirectAsk::React { channel: ANA.into(), message: "1".into(), emoji: "x".into(), on: true });
        s.ask_direct(DirectAsk::MarkRead(ANA.into()));
        s.ask_direct(DirectAsk::Call(ANA.into()));
        s.ask_direct(DirectAsk::Answer(ANA.into()));
        s.ask_direct(DirectAsk::Decline(ANA.into()));
    }
    let lines: Vec<String> = captured().into_iter().filter(|l| l.contains("not built yet")).collect();
    assert_eq!(
        lines,
        ["send", "forget", "react", "mark-read", "call", "answer", "decline"]
            .map(|k| format!("discord web: {k} not built yet"))
    );
    assert!(fake.asked().is_empty(), "none of them reaches the page");
}

#[test]
fn events_are_taken_once_and_nothing_for_what_was_unread_at_the_start() {
    let data = tempfile::tempdir().unwrap();
    let (mut s, fake) = signed_in(data.path());
    fake.page(OK);
    fake.page(&list(&[dm_json(ANA, "Ana Example", 4), dm_json(BO, "Bo Example", 0)]));
    s.pump();
    assert!(s.take_events().is_empty());
    fake.page(&list(&[dm_json(ANA, "Ana Example", 4), dm_json(BO, "Bo Example", 1)]));
    fake.page(r#"{"k":"rings","rings":[{"channel":"700000000000000002","caller":"Bo Example","pic":""}]}"#);
    s.pump();
    let events = s.take_events();
    assert_eq!(events.len(), 2);
    assert!(
        matches!(&events[0], DirectEvent::Message { channel, name, .. } if channel.as_str() == BO && name.as_str() == "Bo Example")
    );
    assert!(matches!(&events[1], DirectEvent::Ring(r) if r.channel.as_str() == BO));
    assert_eq!(s.direct().rings.len(), 1);
    assert_eq!(events[0].toast(), Some("DiscordDm"));
    assert!(s.take_events().is_empty(), "taken once");
    // The open conversation raises nothing.
    s.ask_direct(DirectAsk::Open(ANA.into()));
    fake.page(&list(&[dm_json(ANA, "Ana Example", 5), dm_json(BO, "Bo Example", 1)]));
    fake.page(r#"{"k":"rings","rings":[]}"#);
    s.pump();
    let events = s.take_events();
    assert!(matches!(&events[..], [DirectEvent::RingEnded(id)] if id.as_str() == BO));
    assert!(s.direct().rings.is_empty());
}

#[test]
fn a_web_notification_is_taken_and_raises_nothing_until_its_shape_is_known() {
    let data = tempfile::tempdir().unwrap();
    let (mut s, fake) = signed_in(data.path());
    capturing();
    for _ in 0..3 {
        fake.say(Report::Notified(Notified {
            title: "Ana in #general".into(),
            body: "secret".into(),
            tag: "t".into(),
        }));
    }
    s.pump();
    assert!(s.take_events().is_empty());
    let lines: Vec<String> = captured().into_iter().filter(|l| l.contains("notification")).collect();
    assert_eq!(lines.len(), 1, "said once");
    assert!(!lines[0].contains("Ana") && !lines[0].contains("secret") && lines[0].contains("title 15"));
}

#[test]
fn turning_off_or_signing_out_forgets_everything_and_pictures() {
    let data = tempfile::tempdir().unwrap();
    let (url, _asked) = crate::pictures::tests::cdn(b"made-up picture");
    for how in ["off", "signed out", "page reset"] {
        let (mut s, fake) = signed_in(data.path());
        s.set_cdn(Some(&url));
        fake.page(OK);
        fake.page(&list(&[dm_json(ANA, "Ana Example", 1)]));
        s.ask_direct(DirectAsk::Open(ANA.into()));
        fake.page(&conv(ANA, 2, false, false));
        s.pump();
        assert_eq!(s.direct().dms.len(), 1, "{how}");
        for _ in 0..200 {
            s.pump();
            if s.pictures.held() == 1 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // avatars/9/hash.png is not on the stand-in CDN's served path (avatars/1/): nothing held, nothing wrong.
        match how {
            "off" => s.set_on(false),
            "signed out" => {
                fake.say(Report::State(WebState::SignedOut));
                s.pump();
            }
            _ => {
                fake.page(r#"{"k":"reset"}"#);
                s.pump();
            }
        }
        assert!(s.direct().dms.is_empty() && s.direct().open.is_none() && s.direct().rings.is_empty(), "{how}");
        assert!(s.reader.dms.is_none() && s.reader.conv.is_none() && s.open.is_none(), "{how}");
        assert_eq!(s.pictures.held(), 0, "{how}");
        assert!(s.take_events().is_empty());
    }
}

#[test]
fn pictures_come_by_the_cdn_into_the_model() {
    let data = tempfile::tempdir().unwrap();
    let (url, asked) = crate::pictures::tests::cdn(b"made-up picture");
    let (mut s, fake) = signed_in(data.path());
    s.set_cdn(Some(&url));
    fake.page(OK);
    let one = |id: &str, pic: &str| format!(r#"{{"id":"{id}","name":"N","members":2,"pic":"{pic}","unread":0}}"#);
    fake.page(&list(&[
        one("1", "avatars/1/aa.png?size=128"),
        one("2", ""),
        one("3", "channel-icons/1/bb.png?size=128"),
    ]));
    s.pump();
    for _ in 0..300 {
        s.pump();
        if s.direct().dms[0].picture.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let d = s.direct();
    assert_eq!(&*d.dms[0].picture.as_ref().unwrap().0, b"made-up picture");
    assert!(d.dms[1].picture.is_none(), "no hash: no picture");
    assert!(asked.lock().unwrap().iter().all(|p| p.starts_with("/avatars/1/") || p.starts_with("/channel-icons/1/")));
}

/// The log is places and counts: through a whole session of made-up DMs, no name, id, text or address is in it.
#[test]
fn the_log_has_no_name_id_text_or_address() {
    let data = tempfile::tempdir().unwrap();
    capturing();
    let (mut s, fake) = signed_in(data.path());
    fake.page(OK);
    fake.page(&list(&[dm_json(ANA, "Ana Example", 0), dm_json(BO, "Bo Example", 0)]));
    s.ask_direct(DirectAsk::Open(ANA.into()));
    fake.page(&conv(ANA, 4, true, false));
    s.pump();
    fake.page(&list(&[dm_json(ANA, "Ana Example", 0), dm_json(BO, "Bo Example", 3)]));
    fake.page(r#"{"k":"rings","rings":[{"channel":"700000000000000002","caller":"Bo Example"}]}"#);
    fake.page(r#"{"k":"health","ok":false,"found":{},"missing":["readState.getUnreadCount","messages"]}"#);
    s.pump();
    s.ask_direct(DirectAsk::Send { channel: ANA.into(), text: "secret words".into(), reply: None, nonce: 1 });
    s.ask_direct(DirectAsk::Older);
    s.ask_direct(DirectAsk::Close);
    s.set_cdn(Some("http://127.0.0.1:1"));
    fake.page(&list(&[one_with_pic()]));
    s.pump();
    std::thread::sleep(std::time::Duration::from_millis(300));
    s.pump();
    s.set_on(false);
    let lines = captured();
    assert!(lines.iter().any(|l| l == "discord web: 2 DMs, 0 unread in 0"), "{lines:?}");
    assert!(lines.iter().any(|l| l == "discord web: conversation open, 4 messages"), "{lines:?}");
    assert!(lines.iter().any(|l| l.contains("store \"readState\" no longer answers (getUnreadCount)")), "{lines:?}");
    for line in &lines {
        for private in ["Ana", "Bo ", "Example", "secret", ANA, BO, "http", "avatars", "hash.png"] {
            assert!(!line.contains(private), "{private:?} is in the log line {line:?}");
        }
    }
}

fn one_with_pic() -> String {
    dm_json(ANA, "Ana Example", 0)
}
