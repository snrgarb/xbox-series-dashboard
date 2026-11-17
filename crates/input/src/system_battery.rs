//! Windows' word on the first pad's battery, in the Bundle's order (its GamingInputController.cs and
//! BluetoothBattery.cs; docs/research/pad-battery.md):
//!
//! 1. Windows.Gaming.Input finds the pad and says whether it is on a cable.
//! 2. On Bluetooth LE, the percentage Windows keeps for the device (the one Settings > Bluetooth & devices shows):
//!    Windows.Gaming.Input's own report for such a pad is a placeholder.
//! 3. Else Windows.Gaming.Input's battery report: a level, or a percentage for a pad that reports one.
//! 4. Else, or with no Windows.Gaming.Input pad, XInput's battery information: a cable or a level.
use std::sync::mpsc::Sender;

use ui::Battery;
use windows::Gaming::Input::{Gamepad, RawGameController};
use windows::System::Power::BatteryStatus;
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiGetClassDevsW, SetupDiGetDeviceInstanceIdW,
    SetupDiGetDevicePropertyW, DIGCF_ALLCLASSES, DIGCF_PRESENT, HDEVINFO, SP_DEVINFO_DATA,
};
use windows_sys::Win32::Foundation::DEVPROPKEY;

use super::battery::{from_report, from_xinput, hid_device, News, Source};

/// The battery level Windows keeps for a Bluetooth LE device ({104EA319-6EE2-4701-BD47-8DDBF425BBE5}, 2), and
/// whether the device is connected (DEVPKEY_Device_IsConnected).
const BATTERY_KEY: DEVPROPKEY =
    DEVPROPKEY { fmtid: windows_sys::core::GUID::from_u128(0x104EA319_6EE2_4701_BD47_8DDBF425BBE5), pid: 2 };
const CONNECTED_KEY: DEVPROPKEY =
    DEVPROPKEY { fmtid: windows_sys::core::GUID::from_u128(0x83DA6326_97A6_4088_9453_A1923F573B29), pid: 15 };

/// XInput's battery call, if XInput has one.
pub type XInputBattery = unsafe extern "system" fn(u32, u8, *mut [u8; 2]) -> u32;

pub struct System {
    xinput: Option<XInputBattery>,
    /// Whether Windows.Gaming.Input has failed already: said once.
    failed: bool,
}

impl System {
    /// `again` is told each time Windows.Gaming.Input finds a pad: it finds those already connected a moment after
    /// it is first asked, so the first reading may have had only XInput's word.
    pub fn new(xinput: Option<XInputBattery>, again: Sender<News>) -> Self {
        let handler = windows::Foundation::EventHandler::<Gamepad>::new(move |_, _| {
            let _ = again.send(News::Again);
            Ok(())
        });
        if let Err(e) = Gamepad::GamepadAdded(&handler) {
            eprintln!("gamepad: Windows.Gaming.Input is not there ({e}); the battery is XInput's alone");
        }
        Self { xinput, failed: false }
    }

    fn xinput(&self, slot: u32) -> Option<Battery> {
        let mut information = [0u8; 2];
        // BATTERY_DEVTYPE_GAMEPAD.
        if unsafe { self.xinput?(slot, 0, &mut information) } != 0 {
            return None;
        }
        from_xinput(information[0], information[1])
    }

    /// What Windows.Gaming.Input says of its first pad: None with no pad there; Some(None) for a wireless pad whose
    /// charge it does not know.
    fn gaming_input(&self) -> windows::core::Result<Option<Option<Battery>>> {
        let pads = Gamepad::Gamepads()?;
        if pads.Size()? == 0 {
            return Ok(None);
        }
        let pad = pads.GetAt(0)?;
        if !pad.IsWireless()? {
            return Ok(Some(Some(Battery::Wired)));
        }
        if let Ok(raw) = RawGameController::FromGameController(&pad) {
            if let (Ok(vendor), Ok(product)) = (raw.HardwareVendorId(), raw.HardwareProductId()) {
                if let Some(percent) = bluetooth_le_percent(vendor, product) {
                    return Ok(Some(Some(Battery::Percent(percent.min(100)))));
                }
            }
        }
        let report = pad.TryGetBatteryReport()?;
        if report.Status()? == BatteryStatus::NotPresent {
            return Ok(Some(None));
        }
        let value = |r: windows::core::Result<windows::Foundation::IReference<i32>>| r.and_then(|v| v.Value()).ok();
        let battery = from_report(
            value(report.RemainingCapacityInMilliwattHours()),
            value(report.FullChargeCapacityInMilliwattHours()),
            value(report.DesignCapacityInMilliwattHours()),
        );
        Ok(Some(battery))
    }
}

impl Source for System {
    fn read(&mut self, slot: u32) -> Option<Battery> {
        match self.gaming_input() {
            Ok(Some(Some(battery))) => Some(battery),
            // A wireless pad Windows.Gaming.Input knows no charge of, or no pad it has found yet: XInput's word.
            Ok(Some(None) | None) => self.xinput(slot),
            Err(e) => {
                if !self.failed {
                    self.failed = true;
                    eprintln!("gamepad: Windows.Gaming.Input could not read the battery ({e}); XInput's word instead");
                }
                self.xinput(slot)
            }
        }
    }
}

/// The percentage Windows keeps for the first connected Bluetooth LE device that is a HID device of this vendor and
/// product, if it keeps one.
fn bluetooth_le_percent(vendor: u16, product: u16) -> Option<u8> {
    let mut addresses = Vec::new();
    each_device("BTHLEDEVICE", |_, _, id| {
        if let Some((v, p, address)) = hid_device(id) {
            if (v, p) == (vendor, product) {
                addresses.push(address);
            }
        }
    });
    if addresses.is_empty() {
        return None;
    }
    let mut percent = None;
    each_device("BTHLE", |set, data, id| {
        let upper = id.to_ascii_uppercase();
        let ours = addresses.iter().any(|a| upper.starts_with(&format!(r"BTHLE\DEV_{a}\")));
        if percent.is_none() && ours && byte_property(set, data, &CONNECTED_KEY).is_some_and(|c| c != 0) {
            percent = byte_property(set, data, &BATTERY_KEY);
        }
    });
    percent
}

/// Calls `each` with every present device under this enumerator: the set, the device and its instance id.
fn each_device(enumerator: &str, mut each: impl FnMut(HDEVINFO, &SP_DEVINFO_DATA, &str)) {
    let name: Vec<u16> = enumerator.encode_utf16().chain([0]).collect();
    let set = unsafe {
        SetupDiGetClassDevsW(std::ptr::null(), name.as_ptr(), std::ptr::null_mut(), DIGCF_PRESENT | DIGCF_ALLCLASSES)
    };
    if set == -1 {
        return;
    }
    let mut data =
        SP_DEVINFO_DATA { cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32, ..unsafe { std::mem::zeroed() } };
    let mut buffer = [0u16; 512];
    let mut index = 0;
    while unsafe { SetupDiEnumDeviceInfo(set, index, &mut data) } != 0 {
        let mut length = 0;
        if unsafe { SetupDiGetDeviceInstanceIdW(set, &data, buffer.as_mut_ptr(), buffer.len() as u32, &mut length) }
            != 0
        {
            let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
            each(set, &data, &String::from_utf16_lossy(&buffer[..end]));
        }
        index += 1;
    }
    unsafe { SetupDiDestroyDeviceInfoList(set) };
}

/// A one-byte property (a byte or a boolean) of a device, if it has it.
fn byte_property(set: HDEVINFO, data: &SP_DEVINFO_DATA, key: &DEVPROPKEY) -> Option<u8> {
    let (mut kind, mut buffer, mut size) = (0u32, [0u8; 4], 0u32);
    let ok = unsafe {
        SetupDiGetDevicePropertyW(set, data, key, &mut kind, buffer.as_mut_ptr(), buffer.len() as u32, &mut size, 0)
    };
    (ok != 0 && size >= 1).then_some(buffer[0])
}
