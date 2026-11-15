//! Keep Android awake (Settings → Android Display Options).
//!
//! Android dims the screen after `screen_off_timeout` without a touch (60 s
//! on Waydroid's image) and then turns it off. Waydroid shows the dimmed
//! frame as a grey game window, and with `suspend_action = freeze` it then
//! freezes the whole container, so a game left alone for a minute turns grey
//! and stops. A desktop emulator should not do that: the host already has its
//! own idle and lock handling.
//!
//! On by default. The preference lives on the host, so a fresh Android (after
//! Factory Reset) is kept awake again at the next boot, and the timeout Android
//! had before is remembered so turning the option off restores it.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The largest value `Settings.System` takes: Android never times out.
pub const NEVER: u64 = i32::MAX as u64;

/// Android's own default, used when nothing was remembered.
pub const ANDROID_DEFAULT: u64 = 60_000;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preference {
    pub enabled: bool,
    /// The timeout Android had before ArchDroid raised it.
    pub previous_ms: Option<u64>,
}

impl Default for Preference {
    fn default() -> Self {
        Self {
            enabled: true,
            previous_ms: None,
        }
    }
}

fn preference_path() -> PathBuf {
    dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
        .unwrap_or_else(std::env::temp_dir)
        .join("io.github.archdroid")
        .join("keep-awake.json")
}

/// A missing or unreadable file means the default: kept awake.
fn load(path: &Path) -> Preference {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save(path: &Path, preference: &Preference) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(preference)?)?;
    std::fs::rename(tmp, path)
}

/// Android's timeout as ArchDroid reads and writes it.
pub trait Screen {
    fn timeout(&mut self) -> crate::error::Result<Option<u64>>;
    fn set_timeout(&mut self, ms: u64) -> crate::error::Result<()>;
}

/// `active` lets an explicit user action resume a frozen container; the
/// boot-time sync never does.
struct Adb {
    active: bool,
}

impl Screen for Adb {
    fn timeout(&mut self) -> crate::error::Result<Option<u64>> {
        crate::adb::screen_off_timeout(self.active)
    }
    fn set_timeout(&mut self, ms: u64) -> crate::error::Result<()> {
        crate::adb::set_screen_off_timeout(ms)
    }
}

/// Bring Android in line with `preference`; returns whether it is kept awake
/// and the preference to store (the remembered timeout may have changed).
fn apply(
    screen: &mut dyn Screen,
    mut preference: Preference,
) -> crate::error::Result<(bool, Preference)> {
    let current = screen.timeout()?;
    let awake = current == Some(NEVER);
    if preference.enabled && !awake {
        preference.previous_ms = current;
        screen.set_timeout(NEVER)?;
    } else if !preference.enabled && awake {
        screen.set_timeout(preference.previous_ms.unwrap_or(ANDROID_DEFAULT))?;
    }
    Ok((preference.enabled, preference))
}

/// Called once Android has booted: applies the stored choice.
pub fn sync() -> crate::error::Result<bool> {
    let path = preference_path();
    let (awake, preference) = apply(&mut Adb { active: false }, load(&path))?;
    save(&path, &preference).map_err(crate::error::AppError::Io)?;
    Ok(awake)
}

/// The Settings switch: changes Android first, then remembers the choice.
pub fn set(enabled: bool) -> crate::error::Result<bool> {
    let path = preference_path();
    let preference = Preference {
        enabled,
        ..load(&path)
    };
    let (awake, preference) = apply(&mut Adb { active: true }, preference)?;
    save(&path, &preference).map_err(crate::error::AppError::Io)?;
    Ok(awake)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake {
        timeout: Option<u64>,
        writes: Vec<u64>,
    }

    impl Screen for Fake {
        fn timeout(&mut self) -> crate::error::Result<Option<u64>> {
            Ok(self.timeout)
        }
        fn set_timeout(&mut self, ms: u64) -> crate::error::Result<()> {
            self.timeout = Some(ms);
            self.writes.push(ms);
            Ok(())
        }
    }

    fn fake(timeout: Option<u64>) -> Fake {
        Fake {
            timeout,
            writes: Vec::new(),
        }
    }

    #[test]
    fn on_by_default_and_remembers_the_timeout_it_replaced() {
        let mut screen = fake(Some(60_000));
        let (awake, stored) = apply(&mut screen, Preference::default()).unwrap();
        assert!(awake);
        assert_eq!(screen.writes, vec![NEVER]);
        assert_eq!(stored.previous_ms, Some(60_000));

        // Already awake at the next boot: nothing is written, nothing forgotten.
        let (_, again) = apply(&mut screen, stored).unwrap();
        assert_eq!(screen.writes, vec![NEVER]);
        assert_eq!(again.previous_ms, Some(60_000));

        // Off restores what Android had.
        let off = Preference {
            enabled: false,
            ..again
        };
        let (awake, _) = apply(&mut screen, off).unwrap();
        assert!(!awake);
        assert_eq!(screen.timeout, Some(60_000));
    }

    #[test]
    fn off_without_a_remembered_timeout_uses_androids_default() {
        let mut screen = fake(Some(NEVER));
        let off = Preference {
            enabled: false,
            previous_ms: None,
        };
        apply(&mut screen, off).unwrap();
        assert_eq!(screen.timeout, Some(ANDROID_DEFAULT));

        // Off and already timing out: a timeout the user chose is left alone.
        let mut screen = fake(Some(300_000));
        let off = Preference {
            enabled: false,
            previous_ms: None,
        };
        apply(&mut screen, off).unwrap();
        assert!(screen.writes.is_empty());
    }

    #[test]
    fn preference_file_round_trips_and_defaults_to_awake() {
        let dir = std::env::temp_dir().join(format!("archdroid-awake-{}", std::process::id()));
        let path = dir.join("keep-awake.json");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(load(&path), Preference::default());
        let stored = Preference {
            enabled: false,
            previous_ms: Some(120_000),
        };
        save(&path, &stored).unwrap();
        assert_eq!(load(&path), stored);
        std::fs::write(&path, "not json").unwrap();
        assert!(load(&path).enabled);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
