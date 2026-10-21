//! Whether the machine is running on battery.
//!
//! Upscaling is the heaviest thing anistream asks of a laptop: a CNN chain keeps the GPU
//! busy for every frame of a 24-minute episode. On battery the chooser in
//! [`crate::shaders`] starts one step lighter, which is the difference between finishing a
//! season on a flight and not.
//!
//! The answer is a hint, never a requirement. Anything this module cannot read comes back as
//! `None`, and the caller treats that as mains power — guessing "battery" would quietly
//! degrade the picture on every desktop where the probe fails.
//!
//! **Blocking.** macOS runs `pmset` (about 10 ms) and Linux reads sysfs. Both are fast, but
//! neither belongs on an async worker: call [`PowerSource::on_battery`] from
//! `tokio::task::spawn_blocking`.
//!
//! Behind [`PowerSource`], since only the desktop can ask the system itself: [`SystemPower`] is
//! the desktop's, and a native app reports what its platform says through [`ReportedPower`].

use std::sync::atomic::{AtomicU8, Ordering};

/// Where a playback learns whether the machine is on battery.
pub trait PowerSource: Send + Sync {
    /// `Some(true)` on battery, `Some(false)` on external power, `None` when unknown. May block
    /// for a moment, so it is asked from a blocking thread.
    fn on_battery(&self) -> Option<bool>;
}

/// The desktop's own answer, from the system: see [`on_battery`].
#[cfg(feature = "desktop")]
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemPower;

#[cfg(feature = "desktop")]
impl PowerSource for SystemPower {
    fn on_battery(&self) -> Option<bool> {
        on_battery()
    }
}

/// What the front end last said of the power source, for a platform where only the app can
/// ask: unknown until it says, which reads as external power.
#[derive(Debug, Default)]
pub struct ReportedPower(AtomicU8);

impl ReportedPower {
    const UNKNOWN: u8 = 0;
    const MAINS: u8 = 1;
    const BATTERY: u8 = 2;

    /// What the front end says now.
    pub fn report(&self, on_battery: Option<bool>) {
        let value = match on_battery {
            None => Self::UNKNOWN,
            Some(false) => Self::MAINS,
            Some(true) => Self::BATTERY,
        };
        self.0.store(value, Ordering::Relaxed);
    }
}

impl PowerSource for ReportedPower {
    fn on_battery(&self) -> Option<bool> {
        match self.0.load(Ordering::Relaxed) {
            Self::MAINS => Some(false),
            Self::BATTERY => Some(true),
            _ => None,
        }
    }
}

/// `Some(true)` on battery, `Some(false)` on external power, `None` when unknown.
///
/// - macOS: the first line of `pmset -g batt` names the source ("Battery Power",
///   "AC Power", or "UPS Power" for a desktop on a UPS, which counts as battery: the power
///   is just as finite).
/// - Linux: `/sys/class/power_supply`. External power online wins; otherwise a system
///   battery reporting "Discharging" means battery. Peripheral batteries (a mouse, a
///   headset) carry `scope = Device` and are ignored.
/// - Windows: always `None`. `GetSystemPowerStatus` is the right call and `windows-sys` is
///   already in the dependency tree, but every binding to it is an `unsafe fn` and the
///   workspace forbids `unsafe_code` — `forbid` cannot be lifted for one module. The safe
///   alternatives (PowerShell, CIM) take hundreds of milliseconds.
#[cfg(feature = "desktop")]
pub fn on_battery() -> Option<bool> {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("pmset").args(["-g", "batt"]).output().ok()?;
        if !out.status.success() {
            return None;
        }
        parse_pmset(&String::from_utf8_lossy(&out.stdout))
    }
    #[cfg(target_os = "linux")]
    {
        linux_verdict(&read_supplies(std::path::Path::new("/sys/class/power_supply"))?)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

/// The verdict in `pmset -g batt` output.
///
/// Only the "Now drawing from" line is read: it is the system's own verdict. The
/// per-battery lines that follow describe charge state ("charged", "AC attached; not
/// charging") rather than where the power comes from.
#[cfg(feature = "desktop")]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_pmset(output: &str) -> Option<bool> {
    let line = output.lines().find(|l| l.contains("drawing from"))?;
    let source = line.split('\'').nth(1)?;
    match source {
        "AC Power" => Some(false),
        "Battery Power" | "UPS Power" => Some(true),
        _ => None,
    }
}

/// One entry of `/sys/class/power_supply`, reduced to what the verdict reads.
#[cfg(feature = "desktop")]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Supply {
    /// `type`: `Mains`, `Battery`, `USB`, `Wireless`, `UPS`.
    kind: String,
    /// `online`, for external supplies.
    online: Option<bool>,
    /// `status`, for batteries: `Charging`, `Discharging`, `Full`, `Not charging`, `Unknown`.
    status: Option<String>,
    /// `scope`: `Device` marks a peripheral's battery; absent or `System` is the machine's.
    scope: Option<String>,
}

#[cfg(all(feature = "desktop", target_os = "linux"))]
fn read_supplies(root: &std::path::Path) -> Option<Vec<Supply>> {
    let read = |dir: &std::path::Path, name: &str| {
        std::fs::read_to_string(dir.join(name)).ok().map(|s| s.trim().to_owned())
    };
    let mut supplies = Vec::new();
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        let dir = entry.path();
        let Some(kind) = read(&dir, "type") else { continue };
        supplies.push(Supply {
            kind,
            online: read(&dir, "online").map(|v| v == "1"),
            status: read(&dir, "status"),
            scope: read(&dir, "scope"),
        });
    }
    Some(supplies)
}

/// The verdict over every supply the kernel lists.
///
/// A machine with no system battery at all is a desktop, and a desktop is not on battery:
/// `Some(false)`, not `None`. `None` is left for the case where nothing could be read.
#[cfg(feature = "desktop")]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn linux_verdict(supplies: &[Supply]) -> Option<bool> {
    let external_online = supplies.iter().any(|s| {
        matches!(s.kind.as_str(), "Mains" | "USB" | "Wireless") && s.online == Some(true)
    });
    if external_online {
        return Some(false);
    }
    let discharging = supplies.iter().any(|s| {
        s.kind == "Battery"
            && s.scope.as_deref() != Some("Device")
            && s.status.as_deref() == Some("Discharging")
    });
    Some(discharging)
}

#[cfg(test)]
mod reported_tests {
    use super::*;

    #[test]
    fn what_the_app_reports_is_the_answer_and_unknown_until_it_does() {
        let power = ReportedPower::default();
        assert_eq!(power.on_battery(), None);
        power.report(Some(true));
        assert_eq!(power.on_battery(), Some(true));
        power.report(Some(false));
        assert_eq!(power.on_battery(), Some(false));
        power.report(None);
        assert_eq!(power.on_battery(), None);
    }
}

#[cfg(all(test, feature = "desktop"))]
mod tests {
    use super::*;

    #[test]
    fn pmset_on_ac_with_a_charged_battery() {
        let out = "Now drawing from 'AC Power'\n \
                   -InternalBattery-0 (id=23003235)\t100%; charged; 0:00 remaining present: true\n";
        assert_eq!(parse_pmset(out), Some(false));
    }

    #[test]
    fn pmset_on_battery() {
        let out = "Now drawing from 'Battery Power'\n \
                   -InternalBattery-0 (id=4653155)\t87%; discharging; 5:12 remaining present: true\n";
        assert_eq!(parse_pmset(out), Some(true));
    }

    #[test]
    fn pmset_on_a_desktop_without_a_battery() {
        assert_eq!(parse_pmset("Now drawing from 'AC Power'\n"), Some(false));
    }

    #[test]
    fn pmset_on_a_ups_counts_as_battery() {
        let out = "Now drawing from 'UPS Power'\n \
                   -UPS-0 (id=1)\t96%; discharging; 0:41 remaining present: true\n";
        assert_eq!(parse_pmset(out), Some(true));
    }

    #[test]
    fn pmset_output_that_says_nothing_is_unknown() {
        assert_eq!(parse_pmset(""), None);
        assert_eq!(parse_pmset("Now drawing from 'Solar Power'\n"), None);
    }

    fn supply(kind: &str) -> Supply {
        Supply { kind: kind.into(), ..Supply::default() }
    }

    #[test]
    fn linux_laptop_unplugged() {
        let supplies = [
            Supply { online: Some(false), ..supply("Mains") },
            Supply { status: Some("Discharging".into()), ..supply("Battery") },
        ];
        assert_eq!(linux_verdict(&supplies), Some(true));
    }

    #[test]
    fn linux_laptop_plugged_in_while_the_battery_still_reports_discharging() {
        // A machine drawing more than its adapter supplies drains the battery with the
        // adapter in. It is still plugged in, and mains online is the answer that matters.
        let supplies = [
            Supply { online: Some(true), ..supply("Mains") },
            Supply { status: Some("Discharging".into()), ..supply("Battery") },
        ];
        assert_eq!(linux_verdict(&supplies), Some(false));
    }

    #[test]
    fn linux_usb_c_charging_counts_as_external_power() {
        let supplies = [
            Supply { online: Some(false), ..supply("Mains") },
            Supply { online: Some(true), ..supply("USB") },
            Supply { status: Some("Charging".into()), ..supply("Battery") },
        ];
        assert_eq!(linux_verdict(&supplies), Some(false));
    }

    #[test]
    fn linux_a_discharging_mouse_is_not_the_machine() {
        let supplies = [Supply {
            status: Some("Discharging".into()),
            scope: Some("Device".into()),
            ..supply("Battery")
        }];
        assert_eq!(linux_verdict(&supplies), Some(false));
    }

    #[test]
    fn linux_desktop_with_no_supplies_is_on_mains() {
        assert_eq!(linux_verdict(&[]), Some(false));
    }

    #[test]
    fn linux_full_battery_without_a_mains_entry_is_not_battery() {
        let supplies = [Supply { status: Some("Full".into()), ..supply("Battery") }];
        assert_eq!(linux_verdict(&supplies), Some(false));
    }

    #[test]
    fn the_real_probe_answers_quickly() {
        let started = std::time::Instant::now();
        let verdict = on_battery();
        let took = started.elapsed();
        eprintln!("on_battery() = {verdict:?} in {took:?}");
        // Generous for a loaded CI box; the point is "not a PowerShell launch".
        assert!(took < std::time::Duration::from_millis(500), "took {took:?}");
    }
}
