//! The first pad's battery (docs/spec/home.md 2.4, docs/research/pad-battery.md): read on a thread of its own, as a
//! pad connects and once a minute after, so that a slow answer from Windows never holds up the pad's buttons.
//!
//! Where it comes from is a [`Source`]: Windows' ([`System`], on Windows), or a [`Fake`] that always says the same,
//! for tests and for a payload (`dashboard --fake-pad-battery 85`).
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use ui::Battery;

/// The battery is read again this long after it was last read, while a pad stays connected.
pub const EVERY: Duration = Duration::from_secs(60);

/// What the first pad's battery is. Called on the battery's thread only.
pub trait Source: Send + 'static {
    /// The battery of the first pad connected, `slot` its place among XInput's four. None when nothing is known.
    fn read(&mut self, slot: u32) -> Option<Battery>;
}

/// The same answer whatever the pad.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fake(pub Option<Battery>);

impl Source for Fake {
    fn read(&mut self, _slot: u32) -> Option<Battery> {
        self.0
    }
}

impl Fake {
    /// A fake as `--fake-pad-battery` gives it: a percentage (`85`, `85%`), a level as XInput says one (`empty`,
    /// `low`, `medium`, `full`), `wired`, or `none` for a pad that says nothing.
    pub fn parse(text: &str) -> Result<Self, String> {
        let text = text.trim().to_ascii_lowercase();
        let battery = match text.as_str() {
            "none" => None,
            "wired" => Some(Battery::Wired),
            "empty" => Some(Battery::Level(0)),
            "low" => Some(Battery::Level(1)),
            "medium" => Some(Battery::Level(2)),
            "full" => Some(Battery::Level(3)),
            figure => match figure.trim_end_matches('%').parse::<u8>() {
                Ok(percent) if percent <= 100 => Some(Battery::Percent(percent)),
                _ => {
                    return Err(format!(
                        "bad pad battery {text:?}: a percentage 0 to 100, empty, low, medium, full, wired or none"
                    ))
                }
            },
        };
        Ok(Self(battery))
    }
}

/// What the log says of a reading: `85%`, `level 2 of 3`, `wired`, `not known`.
pub fn describe(battery: Option<Battery>) -> String {
    match battery {
        Some(Battery::Percent(percent)) => format!("{percent}%"),
        Some(Battery::Level(level)) => format!("level {level} of 3"),
        Some(Battery::Wired) => "wired".into(),
        None => "not known".into(),
    }
}

/// What the battery's thread is told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum News {
    /// The first pad connected is now this one, or none is.
    First(Option<u32>),
    /// Windows found a pad: read again, as its answer may be better now.
    Again,
}

/// The battery's thread: waits for news or for the minute, reads when a pad is there, and hands `tell` the first
/// reading of a pad that has just become the first, and after that each that differs from the last. Returns when
/// the news stops coming (the pad's thread has gone).
pub fn run(news: Receiver<News>, mut source: impl Source, mut tell: impl FnMut(Option<Battery>)) {
    let (mut first, mut last) = (None, None);
    loop {
        let arrived = match news.recv_timeout(EVERY) {
            Ok(News::First(pad)) => {
                first = pad;
                pad.is_some()
            }
            Ok(News::Again) | Err(RecvTimeoutError::Timeout) => false,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        let reading = first.and_then(|slot| source.read(slot));
        if arrived || reading != last {
            last = reading;
            tell(reading);
        }
    }
}

/// XInput's battery type and level (`XINPUT_BATTERY_INFORMATION`) as a battery: a cable, or one of its four levels.
/// Disconnected and an unknown type say nothing of the charge.
pub fn from_xinput(kind: u8, level: u8) -> Option<Battery> {
    match (kind, level) {
        // BATTERY_TYPE_WIRED.
        (1, _) => Some(Battery::Wired),
        // Alkaline or NiMH: BATTERY_LEVEL_EMPTY, LOW, MEDIUM, FULL.
        (2 | 3, 0..=3) => Some(Battery::Level(level)),
        // BATTERY_TYPE_DISCONNECTED, BATTERY_TYPE_UNKNOWN.
        _ => None,
    }
}

/// A Windows.Gaming.Input battery report as a battery: its remaining and full capacities, and whether it gives a
/// design capacity. Microsoft's guidance (docs/research/pad-battery.md) is that a pad on low-cost battery hardware
/// gives no design capacity and only ever one of 100, 70, 40 and 10 percent, which are XInput's four levels; one that
/// gives a design capacity reports a real charge.
pub fn from_report(remaining: Option<i32>, full: Option<i32>, design: Option<i32>) -> Option<Battery> {
    let (remaining, full) = (remaining?, full?);
    if full <= 0 || remaining < 0 {
        return None;
    }
    let percent = (i64::from(remaining) * 100 / i64::from(full)).min(100) as u8;
    if design.is_some() {
        return Some(Battery::Percent(percent));
    }
    Some(Battery::Level(match percent {
        0..=10 => 0,
        11..=40 => 1,
        41..=70 => 2,
        _ => 3,
    }))
}

/// A Bluetooth LE HID device's instance id, `BTHLEDEVICE\{00001812-...}_DEV_VID&02045E_PID&0B13_REV&0520_987A14D29A9D\...`:
/// its vendor, its product and its radio's address. None for any other device.
pub fn hid_device(id: &str) -> Option<(u16, u16, String)> {
    let upper = id.to_ascii_uppercase();
    let rest = upper.strip_prefix(r"BTHLEDEVICE\{00001812-")?;
    let rest = &rest[rest.find("}_DEV_VID&02")? + "}_DEV_VID&02".len()..];
    let vendor = u16::from_str_radix(rest.get(..4)?, 16).ok()?;
    let rest = rest.get(4..)?.strip_prefix("_PID&")?;
    let product = u16::from_str_radix(rest.get(..4)?, 16).ok()?;
    let rest = rest.get(4..)?.strip_prefix("_REV&")?;
    let rest = rest.get(4..)?.strip_prefix('_')?;
    let address = rest.get(..12)?;
    if !address.bytes().all(|b| b.is_ascii_hexdigit()) || rest.as_bytes().get(12) != Some(&b'\\') {
        return None;
    }
    Some((vendor, product, address.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    #[test]
    fn a_fake_is_a_percentage_a_level_a_cable_or_nothing() {
        assert_eq!(Fake::parse("85"), Ok(Fake(Some(Battery::Percent(85)))));
        assert_eq!(Fake::parse(" 7% "), Ok(Fake(Some(Battery::Percent(7)))));
        assert_eq!(Fake::parse("Medium"), Ok(Fake(Some(Battery::Level(2)))));
        assert_eq!(Fake::parse("empty"), Ok(Fake(Some(Battery::Level(0)))));
        assert_eq!(Fake::parse("wired"), Ok(Fake(Some(Battery::Wired))));
        assert_eq!(Fake::parse("none"), Ok(Fake(None)));
        assert!(Fake::parse("101").is_err());
        assert!(Fake::parse("half").is_err());
    }

    #[test]
    fn xinput_gives_a_cable_or_a_level() {
        assert_eq!(from_xinput(1, 0), Some(Battery::Wired));
        assert_eq!(from_xinput(2, 0), Some(Battery::Level(0)));
        assert_eq!(from_xinput(3, 3), Some(Battery::Level(3)));
        assert_eq!(from_xinput(0, 3), None, "disconnected");
        assert_eq!(from_xinput(0xFF, 2), None, "a battery of a type XInput does not know says nothing of its charge");
    }

    #[test]
    fn a_report_without_a_design_capacity_is_a_level() {
        // Microsoft's four: 100, 70, 40, 10.
        assert_eq!(from_report(Some(100), Some(100), None), Some(Battery::Level(3)));
        assert_eq!(from_report(Some(70), Some(100), None), Some(Battery::Level(2)));
        assert_eq!(from_report(Some(40), Some(100), None), Some(Battery::Level(1)));
        assert_eq!(from_report(Some(10), Some(100), None), Some(Battery::Level(0)));
        assert_eq!(from_report(Some(1530), Some(2200), Some(2400)), Some(Battery::Percent(69)));
        assert_eq!(from_report(None, Some(100), None), None);
        assert_eq!(from_report(Some(5), Some(0), None), None);
    }

    #[test]
    fn a_bluetooth_le_hid_device_names_its_pad_and_address() {
        let id = r"BTHLEDEVICE\{00001812-0000-1000-8000-00805F9B34FB}_DEV_VID&02045E_PID&0B13_REV&0520_987A14D29A9D\9&1A2B3C&0&001F";
        assert_eq!(hid_device(id), Some((0x045E, 0x0B13, "987A14D29A9D".into())));
        assert_eq!(hid_device(&id.to_ascii_lowercase()), Some((0x045E, 0x0B13, "987A14D29A9D".into())));
        // Another service of the same pad, and a classic Bluetooth device.
        assert_eq!(hid_device(&id.replace("00001812", "0000180F")), None);
        assert_eq!(hid_device(r"BTHENUM\{00001124-0000-1000-8000-00805F9B34FB}_VID&0002054C_PID&09CC\8&1"), None);
    }

    #[test]
    fn the_battery_is_read_as_a_pad_connects_and_said_only_when_it_changes() {
        let (send, news) = channel();
        let readings = std::sync::Arc::new(std::sync::Mutex::new(vec![
            Some(Battery::Level(3)),
            Some(Battery::Level(3)),
            Some(Battery::Level(2)),
        ]));
        struct Script(std::sync::Arc<std::sync::Mutex<Vec<Option<Battery>>>>);
        impl Source for Script {
            fn read(&mut self, _slot: u32) -> Option<Battery> {
                let mut left = self.0.lock().unwrap();
                if left.is_empty() {
                    None
                } else {
                    left.remove(0)
                }
            }
        }
        send.send(News::First(Some(0))).unwrap();
        send.send(News::Again).unwrap();
        send.send(News::Again).unwrap();
        send.send(News::First(None)).unwrap();
        // A pad that says nothing is said of as it connects, though nothing is what there was.
        send.send(News::First(Some(1))).unwrap();
        drop(send);
        let mut told = Vec::new();
        run(news, Script(readings.clone()), |battery| told.push(battery));
        assert_eq!(told, [Some(Battery::Level(3)), Some(Battery::Level(2)), None, None]);
        assert!(readings.lock().unwrap().is_empty(), "read on each piece of news while a pad is there, not after");
    }
}
