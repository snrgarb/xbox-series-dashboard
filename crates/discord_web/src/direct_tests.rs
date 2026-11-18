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
                    Command::Act(op) => (format!("{op:?}"), false),
                    Command::Type { text, clear } => {
                        (format!("Type({} chars{})", text.chars().count(), if clear { ", clear" } else { "" }), false)
                    }
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

// ---- acting (#201)

/// A signed-in session with the list read and `ANA` open (its conversation loaded with three messages).
fn open_ana(data: &Path) -> (Session, Fake) {
    let (mut s, fake) = signed_in(data);
    fake.page(OK);
    fake.page(&list(&[dm_json(ANA, "Ana Example", 0), dm_json(BO, "Bo Example", 0)]));
    s.ask_direct(DirectAsk::Open(ANA.into()));
    fake.page(&conv(ANA, 3, false, false));
    s.pump();
    fake.asked();
    (s, fake)
}

/// The open conversation of three messages with the user's own, `id`, last, in `state`.
fn conv_with(id: &str, state: &str) -> String {
    let base = conv(ANA, 3, false, false);
    let mine = format!(
        r#"{{"id":"{id}","author":{{"id":"99","name":"Me Myself"}},"own":true,"at":1700000009000,"text":"secret hello","state":"{state}"}}"#
    );
    base.replace("]}", &format!(",{mine}]}}"))
}

fn act(json: &str) -> String {
    format!(r#"{{"k":"act",{json}}}"#)
}

fn last(s: &Session) -> (String, Delivery) {
    let m = s.direct().open.as_ref().unwrap().messages.last().unwrap().clone();
    (m.id.as_str().to_string(), m.delivery)
}

fn send(nonce: u64, text: &str) -> DirectAsk {
    DirectAsk::Send { channel: ANA.into(), text: text.into(), reply: None, nonce }
}

#[test]
fn a_send_is_shown_sending_at_once_typed_when_the_page_asks_and_done_when_the_store_shows_it() {
    let data = tempfile::tempdir().unwrap();
    let (mut s, fake) = open_ana(data.path());
    s.ask_direct(send(5, "secret hello"));
    assert_eq!(fake.asked(), vec!["Send(#1, 12 chars)".to_string()]);
    assert_eq!(last(&s), ("5".into(), Delivery::Sending), "shown at once, under the nonce");
    // The page focused the box: the text goes to the host.
    fake.page(&act(r#""seq":1,"stage":"typing","draft":true"#));
    s.pump();
    assert_eq!(fake.asked(), vec!["Type(12 chars, clear)".to_string()]);
    // Discord's own optimistic message is in the store: it is the one shown, under the nonce.
    fake.page(&act(r#""seq":1,"stage":"sending","mid":"5000""#));
    fake.page(&conv_with("5000", "SENDING"));
    s.pump();
    let messages = &s.direct().open.as_ref().unwrap().messages;
    assert_eq!(messages.len(), 4, "not shown twice");
    assert_eq!(last(&s), ("5".into(), Delivery::Sending));
    // Sent: a real id; the nonce becomes it, once.
    fake.page(&conv_with("5001", "SENT"));
    fake.page(&act(r#""seq":1,"stage":"sent","mid":"5001""#));
    capturing();
    s.pump();
    assert_eq!(last(&s), ("5001".into(), Delivery::Sent));
    assert_eq!(s.direct().open.as_ref().unwrap().messages.len(), 4);
    assert_eq!(s.take_events(), vec![DirectEvent::Sent { channel: ANA.into(), nonce: 5, message: "5001".into() }]);
    assert!(
        captured().iter().any(|l| l.starts_with("discord web: sent in ") && l.ends_with(" ms")),
        "{:?}",
        captured()
    );
    assert!(s.take_events().is_empty(), "once");
}

#[test]
fn sends_wait_their_turn_one_at_a_time_and_at_most_one_every_seven_tenths_of_a_second() {
    let data = tempfile::tempdir().unwrap();
    let (mut s, fake) = open_ana(data.path());
    s.ask_direct(send(1, "one"));
    s.ask_direct(send(2, "two"));
    s.ask_direct(DirectAsk::React { channel: ANA.into(), message: "900".into(), emoji: "x".into(), on: true });
    s.ask_direct(DirectAsk::MarkRead(ANA.into()));
    assert_eq!(fake.asked(), vec!["Send(#1, 3 chars)".to_string()], "one at a time");
    let both = s.direct().open.as_ref().unwrap().messages.iter().rev().take(2).map(|m| m.delivery).collect::<Vec<_>>();
    assert_eq!(both, vec![Delivery::Sending, Delivery::Sending], "each shown Sending");
    s.pump();
    assert!(fake.asked().is_empty(), "the first has not finished");
    fake.page(&act(r#""seq":1,"stage":"sent","mid":"777""#));
    s.pump();
    assert!(fake.asked().is_empty(), "paced");
    std::thread::sleep(std::time::Duration::from_millis(750));
    s.pump();
    assert_eq!(fake.asked(), vec!["Send(#2, 3 chars)".to_string()]);
    fake.page(&act(r#""seq":2,"stage":"sent","mid":"778""#));
    s.pump();
    std::thread::sleep(std::time::Duration::from_millis(750));
    s.pump();
    assert_eq!(fake.asked(), vec!["React(#3, on)".to_string()]);
    fake.page(&act(r#""seq":3,"stage":"done""#));
    s.pump();
    std::thread::sleep(std::time::Duration::from_millis(750));
    s.pump();
    assert_eq!(fake.asked(), vec!["Ack(#4)".to_string()]);
}

#[test]
fn a_captcha_a_refusal_and_a_failure_are_told_and_the_bubble_stays_until_forgotten() {
    let data = tempfile::tempdir().unwrap();
    let (mut s, fake) = open_ana(data.path());
    capturing();
    s.ask_direct(send(1, "first"));
    fake.page(&act(r#""seq":1,"stage":"captcha""#));
    s.pump();
    assert!(matches!(&s.take_events()[..], [DirectEvent::Captcha, DirectEvent::NotSent { nonce: 1, .. }]));
    assert_eq!(last(&s), ("1".into(), Delivery::Failed));
    std::thread::sleep(std::time::Duration::from_millis(750));
    s.ask_direct(send(2, "second"));
    s.pump();
    fake.asked();
    fake.page(&act(r#""seq":2,"stage":"sending","mid":"6000""#));
    fake.page(&act(
        r#""seq":2,"stage":"failed","why":"send refused","mid":"6000","reason":"Cannot send messages to this user""#,
    ));
    s.pump();
    let events = s.take_events();
    assert!(matches!(&events[0], DirectEvent::Refused(r) if r.as_str() == "Cannot send messages to this user"));
    assert!(matches!(events[1], DirectEvent::NotSent { nonce: 2, .. }));
    let kinds: Vec<_> = s
        .direct()
        .open
        .as_ref()
        .unwrap()
        .messages
        .iter()
        .rev()
        .take(2)
        .map(|m| (m.id.as_str().to_string(), m.delivery))
        .collect();
    assert_eq!(kinds, vec![("2".to_string(), Delivery::Failed), ("1".to_string(), Delivery::Failed)]);
    // Forget the second: gone from the view, and the page drops Discord's own failed one.
    s.ask_direct(DirectAsk::Forget { channel: ANA.into(), message: "2".into() });
    assert_eq!(fake.asked(), vec!["Forget".to_string()]);
    assert_eq!(last(&s), ("1".into(), Delivery::Failed));
    s.ask_direct(DirectAsk::Forget { channel: ANA.into(), message: "1".into() });
    assert_eq!(s.direct().open.as_ref().unwrap().messages.len(), 3);
    let lines = captured();
    assert!(lines.contains(&"discord web: not sent: captcha".to_string()), "{lines:?}");
    assert!(lines.contains(&"discord web: not sent: refused".to_string()), "{lines:?}");
}

#[test]
fn switching_off_or_signing_out_empties_the_queue_and_each_pending_send_is_not_sent() {
    for how in ["off", "signed out", "reset"] {
        let data = tempfile::tempdir().unwrap();
        let (mut s, fake) = open_ana(data.path());
        s.ask_direct(send(1, "one"));
        s.ask_direct(send(2, "two"));
        s.ask_direct(DirectAsk::MarkRead(ANA.into()));
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
        let nonces: Vec<u64> = s
            .take_events()
            .into_iter()
            .filter_map(|e| if let DirectEvent::NotSent { nonce, .. } = e { Some(nonce) } else { None })
            .collect();
        assert_eq!(nonces, vec![1, 2], "{how}");
        assert!(s.acts.idle(), "{how}");
        assert!(s.direct().open.is_none(), "{how}");
    }
}

#[test]
fn an_ask_while_the_session_is_not_up_is_not_sent_at_once() {
    let data = tempfile::tempdir().unwrap();
    let mut s = Session::new(None, Some(data.path()), || {});
    capturing();
    s.ask_direct(send(4, "secret"));
    assert!(matches!(&s.take_events()[..], [DirectEvent::NotSent { nonce: 4, .. }]));
    assert!(captured().contains(&"discord web: not sent: not signed in".to_string()));
    s.ask_direct(DirectAsk::MarkRead(ANA.into()));
    s.ask_direct(DirectAsk::Call(ANA.into()));
    assert!(captured().contains(&"discord web: join not done: not ready".to_string()));
}

#[test]
fn whether_the_page_read_a_conversation_by_itself_is_said_by_yes_or_no() {
    let data = tempfile::tempdir().unwrap();
    let (mut s, fake) = open_ana(data.path());
    capturing();
    fake.page(r#"{"k":"opened","read":true,"acked":false}"#);
    fake.page(r#"{"k":"opened","read":false,"acked":true}"#);
    s.pump();
    let lines: Vec<_> = captured().into_iter().filter(|l| l.contains("read by the page")).collect();
    assert_eq!(
        lines,
        vec![
            "discord web: conversation open, read by the page yes".to_string(),
            "discord web: conversation open, read by the page no".to_string()
        ]
    );
}

/// The log of a session that acts has no text, id, emoji or reason of Discord's.
#[test]
fn the_log_of_acting_has_no_text_id_emoji_or_reason() {
    let data = tempfile::tempdir().unwrap();
    capturing();
    let (mut s, fake) = open_ana(data.path());
    s.ask_direct(DirectAsk::Send {
        channel: ANA.into(),
        text: "secret hello".into(),
        reply: Some("900".into()),
        nonce: 1,
    });
    fake.page(&act(r#""seq":1,"stage":"typing""#));
    fake.page(&act(r#""seq":1,"stage":"failed","why":"reply action not found""#));
    s.pump();
    std::thread::sleep(std::time::Duration::from_millis(750));
    s.ask_direct(send(2, "secret hello"));
    fake.page(&act(
        r#""seq":2,"stage":"failed","why":"send refused","mid":"6000","reason":"private reason Discord gave""#,
    ));
    s.pump();
    std::thread::sleep(std::time::Duration::from_millis(750));
    s.ask_direct(DirectAsk::React { channel: ANA.into(), message: "900".into(), emoji: "\u{1F525}".into(), on: true });
    fake.page(&act(r#""seq":3,"stage":"failed","why":"not shown""#));
    s.pump();
    s.ask_direct(DirectAsk::Forget { channel: ANA.into(), message: "2".into() });
    s.set_on(false);
    let lines = captured();
    assert!(lines.contains(&"discord web: not sent: reply action not found".to_string()), "{lines:?}");
    assert!(lines.contains(&"discord web: reaction not done: not shown".to_string()), "{lines:?}");
    for line in &lines {
        for private in ["Ana", "Example", "secret", "private reason", ANA, BO, "\u{1F525}", "6000", "900"] {
            assert!(!line.contains(private), "{private:?} is in the log line {line:?}");
        }
    }
    let shown: String = s.take_events().iter().map(|e| format!("{e:?}")).collect();
    assert!(!shown.contains("private reason"), "{shown}");
}

// ---- voice (#297)

const VOICE_OK: &str = r#"{"k":"health","ok":true,"found":{"channels":["ChannelStore","getChannel"],"messages":["MessageStore","getMessages"],"voice":["VoiceStateStore","getVoiceStatesForChannel"],"media":["MediaEngineStore","isSelfMute"],"guilds":["GuildStore","getGuilds"],"voiceActions":["(by shape)","selectVoiceChannel"],"speaking":["SpeakingStore","isSpeaking"]},"missing":[],"voice_missing":[]}"#;

const LOUNGE: &str = "500000000000000001";
const PLACE: &str = "400000000000000001";

/// A Call in a made-up server's channel: the user and one other, who is muted; `joined` is the user's own voice state.
fn call_json(channel: &str, people: usize, joined: bool, mute: bool) -> String {
    let others: Vec<String> = (0..people.saturating_sub(1))
        .map(|i| {
            format!(
                r#"{{"id":"{}","name":"Pal Number{i}","avatar":"hh","muted":true,"deafened":false,"me":false}}"#,
                1 + i
            )
        })
        .collect();
    let mut all = others;
    if people > 0 {
        all.push(r#"{"id":"9","name":"Me Myself","avatar":"","muted":false,"deafened":false,"me":true}"#.into());
    }
    format!(
        r#"{{"k":"voice","me":"Me Myself","channel":"{channel}","title":"Secret Lounge","guild":{{"id":"{PLACE}","name":"Secret Place","icon":"gh"}},"dm":false,"people":[{}],"speaking":["1"],"mute":{mute},"deaf":false,"rtc":{joined},"in":{joined}}}"#,
        all.join(",")
    )
}

fn voice_session(data: &Path) -> (Session, Fake) {
    let (mut s, fake) = signed_in(data);
    fake.page(VOICE_OK);
    fake.page(&list(&[]));
    s.pump();
    (s, fake)
}

fn done(seq: u64) -> String {
    format!(r#"{{"k":"act","seq":{seq},"stage":"done"}}"#)
}

#[test]
fn voice_is_ready_once_its_stores_are_found_and_a_missing_one_is_never_the_dms_break() {
    let data = tempfile::tempdir().unwrap();
    capturing();
    let (mut s, fake) = signed_in(data.path());
    fake.page(OK);
    fake.page(&list(&[dm_json(ANA, "Ana Example", 0)]));
    s.pump();
    assert_eq!((s.direct().state, s.voice_ready()), (DirectState::Ready, false), "the DM stores alone: no voice");
    fake.page(r#"{"k":"health","ok":true,"found":{"channels":["ChannelStore","getChannel"],"guilds":["GuildStore","getGuilds"]},"missing":[],"voice_missing":["voice","voiceActions.selectVoiceChannel"]}"#);
    s.pump();
    assert_eq!(
        (s.direct().state, s.voice_ready()),
        (DirectState::Ready, false),
        "a missing voice store: never PageChanged"
    );
    fake.page(VOICE_OK);
    s.pump();
    assert!(s.voice_ready());
    fake.say(Report::State(WebState::SignedOut));
    s.pump();
    assert!(!s.voice_ready());
    let lines = captured();
    assert!(lines.iter().any(|l| l == "discord web: voice store \"voice\" no longer answers"), "{lines:?}");
    assert!(
        lines.iter().any(|l| l == "discord web: voice store \"voiceActions\" no longer answers (selectVoiceChannel)"),
        "{lines:?}"
    );
    assert!(lines.iter().any(|l| l == "discord web: voice stores found: guilds"), "{lines:?}");
    assert!(
        lines.iter().any(|l| l == "discord web: voice stores found: guilds, media, speaking, voice, voiceActions"),
        "{lines:?}"
    );
    assert!(lines.iter().any(|l| l == "discord web: the voice stores answer again"), "{lines:?}");
}

#[test]
fn the_call_is_the_models_with_the_user_first_and_pump_says_when_it_changed() {
    let data = tempfile::tempdir().unwrap();
    let (mut s, fake) = voice_session(data.path());
    assert!(s.voice().call.is_none() && s.me().is_none() && !s.voice().mute);
    fake.page(&call_json(LOUNGE, 3, true, true));
    assert!(s.pump(), "the Call changed");
    let call = s.voice().call.as_ref().unwrap();
    assert_eq!((call.id.as_str(), call.title.as_str()), (LOUNGE, "Secret Lounge"));
    assert_eq!(
        call.people.iter().map(|p| (p.me, p.speaking, p.muted)).collect::<Vec<_>>(),
        [(true, false, false), (false, true, true), (false, false, true)]
    );
    assert!(s.voice().mute && !s.voice().deaf, "the user's own mute");
    assert_eq!(s.me().map(|m| m.as_str()), Some("Me Myself"));
    assert!(!s.pump(), "nothing more changed");
    fake.page(&call_json("", 0, false, true));
    assert!(s.pump());
    assert!(s.voice().call.is_none() && s.voice().mute, "out of the call, the mute stays");
}

#[test]
fn the_servers_flyout_is_answered_from_what_the_page_pushed_and_back_and_close_are_the_models() {
    use model::discord::{Browse, BrowseAsk};
    let data = tempfile::tempdir().unwrap();
    let (mut s, fake) = voice_session(data.path());
    fake.asked();
    s.ask_voice(Ask::Browse(BrowseAsk::Servers));
    assert!(
        matches!(s.browse(), Browse::Servers { loading: true, servers } if servers.is_empty()),
        "asked before the page said"
    );
    fake.page(&format!(r#"{{"k":"guilds","ordered":false,"servers":[{{"id":"2","name":"beta"}},{{"id":"{PLACE}","name":"Alpha","icon":"gh"}}]}}"#));
    s.pump();
    match s.browse() {
        Browse::Servers { loading: false, servers } => {
            assert_eq!(
                servers.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(),
                ["Alpha", "beta"],
                "by name ignoring case"
            );
            assert!(servers.iter().all(|x| x.icon.is_none()), "no CDN: no picture");
        }
        other => panic!("{other:?}"),
    }
    s.ask_voice(Ask::Browse(BrowseAsk::Server { id: "not digits".into(), name: "x".into() }));
    assert!(matches!(s.browse(), Browse::Servers { .. }), "refused");
    fake.asked();
    s.ask_voice(Ask::Browse(BrowseAsk::Server { id: PLACE.into(), name: "Alpha".into() }));
    assert_eq!(fake.asked(), ["Browse(Some(\"400000000000000001\"))"]);
    assert!(matches!(s.browse(), Browse::Server { loading: true, channels, .. } if channels.is_empty()));
    fake.page(r#"{"k":"server","id":"2","channels":[{"id":"7","name":"Other","type":2,"users":[]}]}"#);
    s.pump();
    assert!(matches!(s.browse(), Browse::Server { loading: true, .. }), "another server's channels are not this one's");
    fake.page(&format!(
        r#"{{"k":"server","id":"{PLACE}","channels":[{{"id":"{LOUNGE}","name":"Lounge","type":2,"users":[{{"id":"1","avatar":"hh"}},{{"id":"2","avatar":""}}]}},{{"id":"8","name":"Stage","type":13,"users":[]}}]}}"#
    ));
    s.pump();
    match s.browse() {
        Browse::Server { id, name, loading: false, channels } => {
            assert_eq!((id.as_str(), name.as_str(), channels.len()), (PLACE, "Alpha", 2));
            assert_eq!(channels[0].occupants.as_ref().unwrap().len(), 2);
            assert_eq!(channels[1].occupants, Some(Vec::new()), "Empty");
        }
        other => panic!("{other:?}"),
    }
    s.ask_voice(Ask::Browse(BrowseAsk::Back));
    assert!(matches!(s.browse(), Browse::Servers { loading: false, .. }), "back to the servers, not asked again");
    assert_eq!(fake.asked(), ["Browse(None)"]);
    s.ask_voice(Ask::Browse(BrowseAsk::Close));
    assert_eq!(*s.browse(), Browse::Closed);
}

#[test]
fn joined_calls_become_shortcuts_kept_in_a_file_of_our_own_and_removed_shortcuts_stay_removed() {
    let data = tempfile::tempdir().unwrap();
    let (mut s, fake) = voice_session(data.path());
    assert!(s.shortcuts().is_empty());
    fake.page(&call_json(LOUNGE, 2, false, false));
    s.pump();
    assert!(s.shortcuts().is_empty(), "connecting is not joined");
    fake.page(&call_json(LOUNGE, 2, true, false));
    s.pump();
    let sc = &s.shortcuts()[0];
    assert_eq!(
        (sc.id.as_str(), sc.name.as_str(), sc.server.as_deref(), sc.dm),
        (LOUNGE, "Secret Lounge", Some("Secret Place"), false)
    );
    let file = data.path().join("discord-web").join("known-profile-7.json");
    assert!(std::fs::read_to_string(&file).unwrap().contains("\"Secret Lounge\""), "kept in the RPC's shape");
    // Back after a restart: read as the session signs in.
    let (mut again, fake2) = voice_session(data.path());
    assert_eq!(again.shortcuts().len(), 1);
    drop(fake2);
    // The Call going on does not bring back a removed one.
    s.ask_voice(Ask::RemoveShortcut(LOUNGE.into()));
    assert!(s.shortcuts().is_empty());
    fake.page(&call_json(LOUNGE, 3, true, false));
    s.pump();
    assert!(s.shortcuts().is_empty());
    assert!(!std::fs::read_to_string(&file).unwrap().contains("Secret Lounge"));
    // Off forgets it from memory only; a sign-out removes the file.
    again.set_on(false);
    assert!(again.shortcuts().is_empty() && file.exists());
    s.host = None;
    s.ask(WebAsk::SignOut);
    assert!(!file.exists(), "a sign-out leaves nothing");
    drop(fake);
}

#[test]
fn the_asks_go_to_the_page_one_at_a_time_paced_and_joining_the_call_one_is_in_does_nothing() {
    use model::discord::CallAsk;
    let data = tempfile::tempdir().unwrap();
    capturing();
    let (mut s, fake) = voice_session(data.path());
    fake.asked();
    s.ask_voice(Ask::Join(LOUNGE.into()));
    s.ask_voice(Ask::Call(CallAsk::Mute));
    assert_eq!(fake.asked(), ["Join(#1)"], "one at a time: the mute waits");
    fake.page(&call_json(LOUNGE, 2, true, false));
    fake.page(&done(1));
    s.pump();
    assert!(captured().iter().any(|l| l.starts_with("discord web: joined in ")), "{:?}", captured());
    // The next goes no sooner than the pace after.
    assert!(fake.asked().is_empty());
    std::thread::sleep(crate::act::PACE + std::time::Duration::from_millis(50));
    s.pump();
    assert_eq!(fake.asked(), ["Mute(#2)"]);
    fake.page(&done(2));
    s.pump();
    s.ask_voice(Ask::Join(LOUNGE.into()));
    assert!(captured().iter().any(|l| l == "discord web: join: already in that call"));
    s.ask_voice(Ask::Call(CallAsk::Leave));
    std::thread::sleep(crate::act::PACE + std::time::Duration::from_millis(50));
    s.pump();
    assert_eq!(fake.asked(), ["Leave(#3)"]);
    // RPC-only asks are ignored.
    s.ask_voice(Ask::Connect);
    s.ask_voice(Ask::ClientId("x".into()));
    assert!(fake.asked().is_empty());
}

#[test]
fn a_dms_call_rings_when_started_joins_when_answered_and_stops_the_ring_when_declined() {
    let data = tempfile::tempdir().unwrap();
    let (mut s, fake) = voice_session(data.path());
    fake.asked();
    s.ask_direct(DirectAsk::Call(ANA.into()));
    assert_eq!(fake.asked(), ["Join(#1, ring)"]);
    fake.page(&done(1));
    s.pump();
    std::thread::sleep(crate::act::PACE + std::time::Duration::from_millis(50));
    s.ask_direct(DirectAsk::Answer(BO.into()));
    assert_eq!(fake.asked(), ["Join(#2)"]);
    fake.page(&done(2));
    s.pump();
    std::thread::sleep(crate::act::PACE + std::time::Duration::from_millis(50));
    s.ask_direct(DirectAsk::Decline(BO.into()));
    assert_eq!(fake.asked(), ["Decline(#3)"]);
    // A ring seen is logged as a state, and the Ring event comes from the rings the page reports.
    capturing();
    fake.page(&format!(r#"{{"k":"rings","rings":[{{"channel":"{BO}","caller":"Bo Example"}}]}}"#));
    fake.page(r#"{"k":"rings","rings":[]}"#);
    s.pump();
    assert_eq!(captured(), ["discord web: ringing", "discord web: ring ended"]);
}

#[test]
fn voice_asks_while_not_ready_are_not_done_and_say_so() {
    let data = tempfile::tempdir().unwrap();
    capturing();
    let (mut s, fake) = signed_in(data.path());
    fake.page(OK);
    fake.page(&list(&[]));
    s.pump();
    fake.asked();
    s.ask_voice(Ask::Join(LOUNGE.into()));
    s.ask_direct(DirectAsk::Call(ANA.into()));
    s.ask_direct(DirectAsk::Decline(ANA.into()));
    assert!(fake.asked().is_empty());
    let lines = captured();
    assert_eq!(lines.iter().filter(|l| l.as_str() == "discord web: join not done: not ready").count(), 2, "{lines:?}");
    assert!(lines.iter().any(|l| l == "discord web: decline not done: not ready"));
}

#[test]
fn turning_off_forgets_the_call_the_flyout_and_who_is_signed_in() {
    use model::discord::BrowseAsk;
    let data = tempfile::tempdir().unwrap();
    let (mut s, fake) = voice_session(data.path());
    fake.page(&call_json(LOUNGE, 2, true, false));
    fake.page(&format!(r#"{{"k":"guilds","ordered":true,"servers":[{{"id":"{PLACE}","name":"Alpha"}}]}}"#));
    s.pump();
    s.ask_voice(Ask::Browse(BrowseAsk::Servers));
    assert!(s.voice().call.is_some() && s.me().is_some() && s.browse().is_open());
    s.set_on(false);
    assert!(s.voice().call.is_none() && s.me().is_none() && !s.browse().is_open() && s.shortcuts().is_empty());
    assert!(s.reader.voice.is_none() && s.reader.guilds.is_none() && !s.voice_ready());
}

#[test]
fn closing_in_a_call_leaves_it_first() {
    let data = tempfile::tempdir().unwrap();
    let (mut s, fake) = voice_session(data.path());
    fake.page(&call_json(LOUNGE, 2, true, false));
    s.pump();
    fake.asked();
    let log = fake.log.clone();
    drop(s);
    assert_eq!(log.lock().unwrap().clone(), ["Leave(#0)", "other"], "Discord's own leave, then the web view is closed");
}

#[test]
fn voice_pictures_are_kept_on_disk_in_the_rpc_crates_art_cache() {
    let data = tempfile::tempdir().unwrap();
    let (url, asked) = crate::pictures::tests::cdn(b"made-up picture");
    let (mut s, fake) = voice_session(data.path());
    s.set_cdn(Some(&url));
    fake.page(&call_json(LOUNGE, 2, true, false));
    s.pump();
    let kept = data.path().join("art").join("discord").join("avatars").join("1").join("hh.png");
    for _ in 0..300 {
        s.pump();
        if s.voice().call.as_ref().is_some_and(|c| c.people.iter().any(|p| p.avatar.as_deref() == Some(kept.as_path())))
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let call = s.voice().call.as_ref().unwrap();
    assert_eq!(call.people[1].avatar.as_deref(), Some(kept.as_path()));
    assert_eq!(std::fs::read(&kept).unwrap(), b"made-up picture", "on disk, as the tab draws it");
    assert!(asked.lock().unwrap().contains(&"/avatars/1/hh.png?size=64".to_string()));
    // Used at once the next time, with no CDN.
    let (mut again, fake2) = voice_session(data.path());
    fake2.page(&call_json(LOUNGE, 2, true, false));
    again.pump();
    assert_eq!(again.voice().call.as_ref().unwrap().people[1].avatar.as_deref(), Some(kept.as_path()));
}

#[test]
fn the_log_of_voice_has_no_name_id_or_address() {
    use model::discord::{BrowseAsk, CallAsk};
    let data = tempfile::tempdir().unwrap();
    capturing();
    let (mut s, fake) = voice_session(data.path());
    fake.page(&call_json(LOUNGE, 3, true, true));
    fake.page(&format!(r#"{{"k":"guilds","ordered":true,"servers":[{{"id":"{PLACE}","name":"Secret Place"}}]}}"#));
    s.pump();
    s.ask_voice(Ask::Join("500000000000000009".into()));
    s.ask_voice(Ask::Call(CallAsk::Deafen));
    s.ask_voice(Ask::Browse(BrowseAsk::Server { id: PLACE.into(), name: "Secret Place".into() }));
    s.ask_voice(Ask::RemoveShortcut(LOUNGE.into()));
    fake.page(&call_json("", 0, false, false));
    s.pump();
    fake.page(r#"{"k":"act","seq":1,"stage":"failed","why":"not connected"}"#);
    s.pump();
    s.set_on(false);
    let lines = captured();
    assert!(lines.iter().any(|l| l == "discord web: in a call, 3 people"), "{lines:?}");
    assert!(lines.iter().any(|l| l == "discord web: left the call"), "{lines:?}");
    assert!(lines.iter().any(|l| l == "discord web: join not done: not connected"), "{lines:?}");
    for line in &lines {
        for private in ["Secret", "Pal", "Me Myself", LOUNGE, PLACE, "50000", "http", "avatars"] {
            assert!(!line.contains(private), "{private:?} is in the log line {line:?}");
        }
    }
}
