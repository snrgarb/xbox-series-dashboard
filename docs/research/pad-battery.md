# A pad's battery on Windows: who says what (#191)

Written 2026-11-17 for #191, from Microsoft's documentation and the Bundle's code (`extension/XboxSeries.Extension/GamingInputController.cs`, `BluetoothBattery.cs`). **Nothing here was tried on real hardware**: the box this was written on has no pad and no Windows, and the test VM's pad is a virtual one. The user's own Bluetooth pad on the TV PC is the check.

## The three places Windows keeps it

| Where | What it gives | Documentation |
|---|---|---|
| XInput, `XInputGetBatteryInformation(slot, BATTERY_DEVTYPE_GAMEPAD)` | The battery's type (disconnected, wired, alkaline, NiMH, unknown) and, for a known type, one of four levels: empty (0–10 %), low (10–40 %), medium (40–70 %), full (70–100 %). Never a percentage | [XInputGetBatteryInformation](https://learn.microsoft.com/windows/win32/api/xinput/nf-xinput-xinputgetbatteryinformation), [XINPUT_BATTERY_INFORMATION](https://learn.microsoft.com/windows/win32/api/xinput/ns-xinput-xinput_battery_information); the levels' bands are in the GDK's copy of the struct, [XINPUT_BATTERY_INFORMATION (GDK)](https://learn.microsoft.com/gaming/gdk/docs/reference/input/xinputongameinput/structs/xinput_battery_information) |
| Windows.Gaming.Input, `Gamepad.TryGetBatteryReport()` | A `BatteryReport`: remaining and full capacity, from which a percentage is worked out. Microsoft says that most pads "use low-cost hardware": for them the design capacity and charge rate are null, and the percentage is only ever 100, 70, 40 or 10, the same four levels. `Gamepad` is any GIP or XUSB pad, and `IsWireless` says whether it is on a cable | [Input practices for games: Get the state of the battery](https://learn.microsoft.com/windows/uwp/gaming/input-practices-for-games#get-the-state-of-the-battery), [Gamepad.TryGetBatteryReport](https://learn.microsoft.com/uwp/api/windows.gaming.input.gamepad.trygetbatteryreport), [BatteryReport](https://learn.microsoft.com/uwp/api/windows.devices.power.batteryreport), [Gamepad](https://learn.microsoft.com/uwp/api/windows.gaming.input.gamepad), [IGameController.IsWireless](https://learn.microsoft.com/uwp/api/windows.gaming.input.igamecontroller.iswireless) |
| The Bluetooth LE device's own property, `{104EA319-6EE2-4701-BD47-8DDBF425BBE5}, 2` on its `BTHLE\DEV_<address>` node, read with SetupAPI | The percentage Settings > Bluetooth & devices shows, from the pad's GATT Battery Service. **Not documented by Microsoft** as a property key: it is what the Bundle reads, and what Windows' own Settings shows | [SetupDiGetDevicePropertyW](https://learn.microsoft.com/windows/win32/api/setupapi/nf-setupapi-setupdigetdevicepropertyw) (the call); the key itself is the Bundle's finding |

The Bundle found (its `BluetoothBattery.cs`) that for an Xbox pad on Bluetooth LE Windows.Gaming.Input's report is "a fixed placeholder" and XInput reports no battery; so it reads the Bluetooth property first, matching the Windows.Gaming.Input pad's vendor and product ids (`RawGameController.HardwareVendorId`/`HardwareProductId`) to a connected Bluetooth LE HID device.

## Which pads answer which

| Pad | Windows.Gaming.Input | XInput | Bluetooth LE property | Shown | Source |
|---|---|---|---|---|---|
| Xbox Wireless Controller over **Bluetooth** (Series, and One with the newer firmware) | wireless; report a placeholder | no battery | **the percentage** | the cell at that charge, and the figure | the Bundle's finding; Microsoft for the LE property's use in Settings is not documented |
| Xbox pad on a **USB cable** | wired | wired | — | the controller glyph | Microsoft (`IsWireless`, `BATTERY_TYPE_WIRED`) |
| Xbox pad on the **Xbox Wireless Adapter** (GIP over Microsoft's own radio) | wireless; a level (100/70/40/10) | a level | — | the cell at that level, no figure | Microsoft documents the levels for wireless pads; **not tried**. The user expected the plain glyph here (#191): if the adapter answers with a level, it shows a level |
| Xbox 360 wireless pad on its receiver (XUSB) | wireless; a level | a level | — | the cell at that level | as above |
| A **virtual** pad (ViGEm's Xbox 360 pad, a streaming host's) | XUSB, wired | wired | — | the glyph | the test VM's pad; ViGEm reports a wired pad |
| An 8BitDo or other pad on its **2.4 GHz dongle** in XInput mode | XUSB, as a wired Xbox 360 pad | wired | — | the glyph | how such dongles present themselves; not tried |
| A pad with real battery reporting (design capacity given) | wireless; a true percentage | — | — | the cell and the figure | Microsoft (`DesignCapacityInMilliwattHours` non-null) |
| DualSense, DualShock 4, Switch pads, and anything else that is not XInput | not a `Gamepad` | not seen | — | nothing: the Dashboard reads pads through XInput, and does not count it as connected | |

## What the Dashboard does with it

`crates/input/src/battery.rs` and `system_battery.rs` (Windows), in the Bundle's order:

1. Windows.Gaming.Input's first `Gamepad`: on a cable, `Wired`.
2. Wireless: the Bluetooth LE percentage of a connected Bluetooth LE HID device with its vendor and product, if Windows keeps one: `Percent`.
3. Else its battery report: with a design capacity, `Percent`; without, the band its percentage falls in, `Level` (0 empty .. 3 full); a report that says no battery, nothing.
4. Else, and when Windows.Gaming.Input has no pad yet (it finds the pads already connected a moment after it is first asked, and says so with `GamepadAdded`, which reads again), XInput's word for the first connected slot: `Wired` or `Level`.

It is read on a thread of its own, never the pad's (which reads the buttons every 4 ms) nor the window's: as the first pad connects, when Windows.Gaming.Input finds a pad, and once a minute while a pad stays (the Bundle's minute). XInput's own battery call costs nothing, but the SetupAPI walk and Windows.Gaming.Input are not free, and a minute is what the Bundle asks. The thread tells the window what it read as a pad connects, and after that only when the answer changes.

The Bundle's other moment, 500 ms after the Guide opens, is not copied: the minute and the connect cover it, and a read as the Guide opens over a game is exactly the moment a game would pay for one.

`dashboard --fake-pad-battery <85 | empty | low | medium | full | wired | none>` puts a `Fake` in place of Windows' word, for a payload (the VM's pad is virtual and on a "cable").

## Low battery

The Bundle has no low-battery Toast (nothing in its source posts one), so the Dashboard has none either.
