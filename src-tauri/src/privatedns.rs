//! AdGuard DNS (Install Tools → Block Ads): Android's Private DNS set to
//! AdGuard's resolver, `dns.adguard-dns.com`.
//!
//! The hosts file (`adblock.rs`) only blocks the names on its list, and games
//! still showed ads through SDKs on other domains. With Private DNS set to
//! AdGuard, AdGuard's servers filter every name Android looks up. That sends
//! every DNS query Android makes to a third party, so it is a separate switch
//! and off by default.
//!
//! Private DNS is two `Settings.Global` values, the same two Android's own
//! Settings → Private DNS screen writes, set over adb without root and without
//! restarting the container.
//!
//! Like Keep Android Awake (`awake.rs`), the choice lives on the host and is
//! applied once Android has booted, so a fresh Android after Factory Reset gets
//! it again, and the values Android had before are remembered so turning it
//! off restores them. A Private DNS server ArchDroid did not set is never
//! replaced or deleted.

use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};

use crate::adb::GlobalWrite;
use crate::awake::{load, preference_path, save};
use crate::error::Result;

pub const MODE: &str = "private_dns_mode";
pub const SPECIFIER: &str = "private_dns_specifier";
const KEYS: [&str; 2] = [MODE, SPECIFIER];

/// Android's mode for "use this server" (strict DNS-over-TLS).
pub const HOSTNAME: &str = "hostname";

/// AdGuard DNS, default servers (ads and trackers).
pub const ADGUARD: &str = "dns.adguard-dns.com";

const FILE: &str = "private-dns.json";

/// Android's Private DNS settings. `None` is a setting never written.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PrivateDns {
    pub mode: Option<String>,
    pub specifier: Option<String>,
}

impl PrivateDns {
    fn adguard() -> Self {
        Self {
            mode: Some(HOSTNAME.to_string()),
            specifier: Some(ADGUARD.to_string()),
        }
    }

    /// Someone picked a server. Off and Automatic (`opportunistic`) are not a
    /// choice of server, so ArchDroid may replace them and put them back.
    fn has_server(&self) -> bool {
        self.mode.as_deref() == Some(HOSTNAME)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preference {
    pub enabled: bool,
    /// What Android had before ArchDroid set AdGuard DNS. `Some` only while
    /// ArchDroid's values are in Android, which is also how ArchDroid tells
    /// them apart from the same values set by hand.
    pub previous: Option<PrivateDns>,
}

/// Android's Private DNS as the Block Ads card shows it.
#[derive(Debug, PartialEq)]
pub enum State {
    On,
    Off,
    /// A server ArchDroid did not set, which it leaves alone.
    Foreign(PrivateDns),
}

impl State {
    pub fn name(&self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Off => "off",
            Self::Foreign(_) => "foreign",
        }
    }

    /// What the card says about a server ArchDroid did not set.
    pub fn message(&self) -> Option<String> {
        let Self::Foreign(current) = self else {
            return None;
        };
        let server = current.specifier.as_deref().unwrap_or("no hostname");
        Some(format!(
            "Android already uses Private DNS {server}, which ArchDroid did not set, so ArchDroid leaves it alone. \
             To use this switch, set Private DNS to Off or Automatic in Android's settings; ArchDroid checks again when Android next starts."
        ))
    }
}

/// Android's Private DNS as ArchDroid reads and writes it.
pub trait Settings {
    fn read(&mut self) -> Result<PrivateDns>;
    fn write(&mut self, writes: &[GlobalWrite]) -> Result<()>;
}

/// `active` lets an explicit user action resume a frozen container; the
/// boot-time sync never does.
struct Adb {
    active: bool,
}

impl Settings for Adb {
    fn read(&mut self) -> Result<PrivateDns> {
        let mut values = crate::adb::global_settings(&KEYS, self.active)?.into_iter();
        Ok(PrivateDns {
            mode: values.next().flatten(),
            specifier: values.next().flatten(),
        })
    }
    fn write(&mut self, writes: &[GlobalWrite]) -> Result<()> {
        crate::adb::set_global_settings(writes)
    }
}

/// The writes that turn AdGuard DNS on. The server goes first: with the mode
/// first, Android would briefly be in strict mode with the old hostname.
fn writes_on() -> [GlobalWrite<'static>; 2] {
    [(SPECIFIER, Some(ADGUARD)), (MODE, Some(HOSTNAME))]
}

/// The writes that put back what Android had, mode first so Android leaves
/// strict mode before the hostname changes. An unset value is deleted again.
fn writes_restore(previous: &PrivateDns) -> [GlobalWrite<'_>; 2] {
    [
        (MODE, previous.mode.as_deref()),
        (SPECIFIER, previous.specifier.as_deref()),
    ]
}

/// Bring Android in line with `preference`; returns what Android uses now and
/// the preference to store. `remember` stores what Android had before AdGuard
/// DNS replaces it, so a write or a save that fails afterwards cannot lose what
/// turning the switch off has to put back.
fn apply(
    android: &mut dyn Settings,
    mut preference: Preference,
    remember: &mut dyn FnMut(&Preference) -> Result<()>,
) -> Result<(State, Preference)> {
    let current = android.read()?;
    let ours = preference.previous.is_some() && current == PrivateDns::adguard();
    if !ours {
        // ArchDroid's values are not in Android (any more), so what it
        // remembered is not what turning off would put back.
        preference.previous = None;
        if current.has_server() {
            // Chosen in Android's settings, AdGuard set by hand included, or
            // over ArchDroid's: the switch reads off until that server is gone.
            preference.enabled = false;
            return Ok((State::Foreign(current), preference));
        }
    }
    let state = match (preference.enabled, &preference.previous) {
        (true, Some(_)) => State::On,
        (true, None) => {
            preference.previous = Some(current);
            remember(&preference)?;
            android.write(&writes_on())?;
            State::On
        }
        (false, Some(previous)) => {
            android.write(&writes_restore(previous))?;
            preference.previous = None;
            State::Off
        }
        (false, None) => State::Off,
    };
    Ok((state, preference))
}

/// Held from reading the file to writing it: a sync after boot and a click on
/// the switch must not both start from the old file.
static CHANGE: Mutex<()> = Mutex::new(());

fn change(
    android: &mut dyn Settings,
    choose: impl FnOnce(Preference) -> Preference,
) -> Result<State> {
    let _change = CHANGE.lock().unwrap_or_else(PoisonError::into_inner);
    let path = preference_path(FILE);
    let stored: Preference = load(&path);
    let (state, next) = apply(android, choose(stored.clone()), &mut |preference| {
        Ok(save(&path, preference)?)
    })?;
    // Never touched, never written: no file for users who leave it off.
    if next != stored {
        save(&path, &next)?;
    }
    Ok(state)
}

/// Called once Android has booted: applies the stored choice.
pub fn sync() -> Result<State> {
    change(&mut Adb { active: false }, |stored| stored)
}

/// The Block Ads card's switch: changes Android first, then remembers the choice.
pub fn set(enabled: bool) -> Result<State> {
    change(&mut Adb { active: true }, |stored| Preference {
        enabled,
        ..stored
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Android's two settings, and each `settings` line ArchDroid sent.
    struct Fake {
        now: PrivateDns,
        lines: Vec<String>,
        /// adb going away before the write.
        failing: bool,
    }

    fn keep(_: &Preference) -> Result<()> {
        Ok(())
    }

    impl Settings for Fake {
        fn read(&mut self) -> Result<PrivateDns> {
            Ok(self.now.clone())
        }
        fn write(&mut self, writes: &[GlobalWrite]) -> Result<()> {
            if self.failing {
                return Err(crate::error::AppError::command_failed("adb went away"));
            }
            for (key, value) in writes {
                let slot = match *key {
                    MODE => &mut self.now.mode,
                    SPECIFIER => &mut self.now.specifier,
                    other => panic!("unexpected setting {other}"),
                };
                *slot = value.map(str::to_string);
            }
            self.lines
                .push(crate::adb::global_settings_write_line(writes));
            Ok(())
        }
    }

    fn dns(mode: Option<&str>, specifier: Option<&str>) -> PrivateDns {
        PrivateDns {
            mode: mode.map(str::to_string),
            specifier: specifier.map(str::to_string),
        }
    }

    fn android(now: PrivateDns) -> Fake {
        Fake {
            now,
            lines: Vec::new(),
            failing: false,
        }
    }

    const ON_LINE: &str = "settings put global private_dns_specifier 'dns.adguard-dns.com' && settings put global private_dns_mode 'hostname'";

    #[test]
    fn what_android_had_is_remembered_before_anything_is_written() {
        let before = dns(Some("opportunistic"), None);
        let mut android = android(before.clone());
        android.failing = true;
        let mut remembered = None;
        let on = Preference {
            enabled: true,
            previous: None,
        };
        let result = apply(&mut android, on, &mut |preference| {
            remembered = Some(preference.clone());
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(remembered.unwrap().previous, Some(before));
    }

    #[test]
    fn off_by_default_and_nothing_is_written() {
        let mut android = android(dns(None, None));
        let (state, stored) = apply(&mut android, Preference::default(), &mut keep).unwrap();
        assert_eq!(state, State::Off);
        assert!(android.lines.is_empty());
        assert_eq!(stored, Preference::default());
    }

    #[test]
    fn on_sets_adguard_and_off_deletes_settings_that_were_never_written() {
        let mut android = android(dns(None, None));
        let on = Preference {
            enabled: true,
            previous: None,
        };
        let (state, stored) = apply(&mut android, on, &mut keep).unwrap();
        assert_eq!(state, State::On);
        assert_eq!(android.lines, [ON_LINE]);
        assert_eq!(android.now, PrivateDns::adguard());
        assert_eq!(stored.previous, Some(dns(None, None)));

        // Already on at the next boot: nothing is written, nothing forgotten.
        let (state, stored) = apply(&mut android, stored, &mut keep).unwrap();
        assert_eq!(state, State::On);
        assert_eq!(android.lines.len(), 1);
        assert_eq!(stored.previous, Some(dns(None, None)));

        // Off: both settings were unset (`null`), so both are deleted again.
        let off = Preference {
            enabled: false,
            ..stored
        };
        let (state, stored) = apply(&mut android, off, &mut keep).unwrap();
        assert_eq!(state, State::Off);
        assert_eq!(
            android.lines[1],
            "settings delete global private_dns_mode && settings delete global private_dns_specifier"
        );
        assert_eq!(android.now, dns(None, None));
        assert_eq!(stored, Preference::default());
    }

    #[test]
    fn off_restores_the_exact_values_android_had() {
        // Automatic, with a hostname left over from an earlier choice.
        let before = dns(Some("opportunistic"), Some("dns.example.org"));
        let mut android = android(before.clone());
        let on = Preference {
            enabled: true,
            previous: None,
        };
        let (_, stored) = apply(&mut android, on, &mut keep).unwrap();
        let off = Preference {
            enabled: false,
            ..stored
        };
        apply(&mut android, off, &mut keep).unwrap();
        assert_eq!(android.now, before);
        assert_eq!(
            android.lines[1],
            "settings put global private_dns_mode 'opportunistic' && settings put global private_dns_specifier 'dns.example.org'"
        );
    }

    #[test]
    fn a_private_dns_archdroid_did_not_set_is_never_replaced_or_deleted() {
        // The user's own server, and AdGuard set by hand in Android's settings.
        for own in [
            dns(Some("hostname"), Some("dns.nextdns.io")),
            PrivateDns::adguard(),
        ] {
            for enabled in [true, false] {
                let mut android = android(own.clone());
                let stored = Preference {
                    enabled,
                    previous: None,
                };
                let (state, stored) = apply(&mut android, stored, &mut keep).unwrap();
                assert_eq!(state, State::Foreign(own.clone()));
                assert!(android.lines.is_empty(), "{own:?} was changed");
                assert_eq!(android.now, own);
                assert_eq!(stored, Preference::default());
            }
        }
        let message = State::Foreign(dns(Some("hostname"), Some("dns.nextdns.io")))
            .message()
            .unwrap();
        assert!(message.contains("dns.nextdns.io"), "{message}");
        assert_eq!(State::On.message(), None);
    }

    #[test]
    fn a_server_chosen_over_archdroids_is_kept_and_the_old_values_forgotten() {
        let mut android = android(dns(Some("hostname"), Some("dns.nextdns.io")));
        let stored = Preference {
            enabled: true,
            previous: Some(dns(Some("off"), None)),
        };
        let (state, stored) = apply(&mut android, stored, &mut keep).unwrap();
        assert_eq!(state.name(), "foreign");
        assert!(android.lines.is_empty());
        assert_eq!(stored, Preference::default());
    }

    #[test]
    fn a_fresh_android_after_factory_reset_gets_adguard_again() {
        let mut android = android(dns(None, None));
        let stored = Preference {
            enabled: true,
            previous: Some(dns(Some("off"), None)),
        };
        let (state, stored) = apply(&mut android, stored, &mut keep).unwrap();
        assert_eq!(state, State::On);
        assert_eq!(android.lines, [ON_LINE]);
        // Off now restores what this Android had, not the wiped one.
        assert_eq!(stored.previous, Some(dns(None, None)));
    }

    #[test]
    fn preference_file_round_trips_and_defaults_to_off() {
        let dir = std::env::temp_dir().join(format!("archdroid-dns-{}", std::process::id()));
        let path = dir.join(FILE);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(load::<Preference>(&path), Preference::default());
        // "Never written" is remembered as such, apart from "nothing remembered".
        let stored = Preference {
            enabled: true,
            previous: Some(dns(None, Some("dns.example.org"))),
        };
        save(&path, &stored).unwrap();
        assert_eq!(load::<Preference>(&path), stored);
        std::fs::write(&path, "not json").unwrap();
        assert!(!load::<Preference>(&path).enabled);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
