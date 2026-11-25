//! Keep the Super key away from Google Assistant (Settings → Android Display
//! Options).
//!
//! Waydroid passes the desktop's Super key to Android as Meta, and Android
//! opens the assistant (Gemini) when Meta is pressed alone. On a desktop where
//! Super starts every shortcut, that opens Gemini over the game, the game
//! pauses, and PvZ2 for one kept a pale veil until it was restarted.
//!
//! On by default. Android's choice is the same as Settings → Apps → Default
//! apps → Digital assistant → None (`assistant` and
//! `voice_interaction_service` empty); the assistant it had is remembered so
//! turning the option off brings it back. The preference lives on the host, so
//! a fresh Android (after Factory Reset) gets it again at the next boot.

use serde::{Deserialize, Serialize};

use crate::awake::{config_file, load, save};
use crate::error::Result;

const ASSISTANT: &str = "assistant";
const VOICE_INTERACTION: &str = "voice_interaction_service";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preference {
    pub blocked: bool,
    /// The assistant Android had before ArchDroid cleared it.
    pub previous_assistant: Option<String>,
    pub previous_voice_interaction: Option<String>,
}

impl Default for Preference {
    fn default() -> Self {
        Self {
            blocked: true,
            previous_assistant: None,
            previous_voice_interaction: None,
        }
    }
}

/// Android's secure settings as ArchDroid reads and writes them. `None` is an
/// unset or empty value (no assistant).
pub trait Secure {
    fn get(&mut self, key: &str) -> Result<Option<String>>;
    fn put(&mut self, key: &str, value: &str) -> Result<()>;
}

/// `active` lets an explicit user action resume a frozen container; the
/// boot-time sync never does.
struct Adb {
    active: bool,
}

impl Secure for Adb {
    fn get(&mut self, key: &str) -> Result<Option<String>> {
        let args = ["settings", "get", "secure", key];
        let output = if self.active {
            crate::adb::shell_active(&args)?
        } else {
            crate::adb::shell(&args)?
        };
        Ok(setting_value(&output))
    }

    fn put(&mut self, key: &str, value: &str) -> Result<()> {
        // One shell line; the value is quoted so an empty one stays an argument.
        let line = format!(
            "settings put secure {key} {}",
            crate::elevate::sh_quote(value)
        );
        crate::adb::shell_active(&[&line]).map(|_| ())
    }
}

/// `settings get` prints `null` for a setting that was never written.
fn setting_value(output: &str) -> Option<String> {
    let value = output.trim();
    (!value.is_empty() && value != "null").then(|| value.to_string())
}

/// Bring Android in line with `preference`; returns whether the Super key is
/// kept from the assistant and the preference to store. `remember` stores the
/// assistant before it is cleared, so a write that fails halfway cannot lose
/// what turning the option off has to bring back.
fn apply(
    secure: &mut dyn Secure,
    mut preference: Preference,
    remember: &mut dyn FnMut(&Preference) -> Result<()>,
) -> Result<(bool, Preference)> {
    let current = secure.get(ASSISTANT)?;
    if preference.blocked {
        if current.is_some() {
            preference.previous_voice_interaction = secure.get(VOICE_INTERACTION)?;
            preference.previous_assistant = current;
            remember(&preference)?;
            secure.put(ASSISTANT, "")?;
            secure.put(VOICE_INTERACTION, "")?;
        }
    } else if current.is_none() {
        if let Some(previous) = &preference.previous_assistant {
            secure.put(ASSISTANT, previous)?;
            if let Some(voice) = &preference.previous_voice_interaction {
                secure.put(VOICE_INTERACTION, voice)?;
            }
        }
    }
    Ok((preference.blocked, preference))
}

fn preference_path() -> std::path::PathBuf {
    config_file("super-key.json")
}

/// Called once Android has booted: applies the stored choice.
pub fn sync() -> Result<bool> {
    let path = preference_path();
    let (blocked, preference) =
        apply(&mut Adb { active: false }, load(&path), &mut |preference| {
            Ok(save(&path, preference)?)
        })?;
    save(&path, &preference)?;
    Ok(blocked)
}

/// The Settings switch: changes Android, then remembers the choice.
pub fn set(blocked: bool) -> Result<bool> {
    let path = preference_path();
    let preference = Preference {
        blocked,
        ..load(&path)
    };
    let (blocked, preference) = apply(&mut Adb { active: true }, preference, &mut |preference| {
        Ok(save(&path, preference)?)
    })?;
    save(&path, &preference)?;
    Ok(blocked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const GSA: &str =
        "com.google.android.googlequicksearchbox/com.google.android.voiceinteraction.GsaVoiceInteractionService";

    #[derive(Default)]
    struct Fake {
        values: HashMap<String, String>,
        writes: usize,
        /// The write (1 = the first) that fails, as adb can halfway through.
        failing_write: Option<usize>,
    }

    impl Secure for Fake {
        fn get(&mut self, key: &str) -> Result<Option<String>> {
            Ok(self.values.get(key).and_then(|v| setting_value(v)))
        }
        fn put(&mut self, key: &str, value: &str) -> Result<()> {
            if self.failing_write == Some(self.writes + 1) {
                return Err(crate::error::AppError::command_failed("adb went away"));
            }
            self.values.insert(key.to_string(), value.to_string());
            self.writes += 1;
            Ok(())
        }
    }

    fn forget(_: &Preference) -> Result<()> {
        Ok(())
    }

    fn with_assistant() -> Fake {
        let mut fake = Fake::default();
        fake.values.insert(ASSISTANT.into(), GSA.into());
        fake.values.insert(VOICE_INTERACTION.into(), GSA.into());
        fake
    }

    #[test]
    fn on_by_default_clears_the_assistant_and_remembers_it() {
        let mut android = with_assistant();
        let (blocked, stored) = apply(&mut android, Preference::default(), &mut forget).unwrap();
        assert!(blocked);
        assert_eq!(android.get(ASSISTANT).unwrap(), None);
        assert_eq!(android.get(VOICE_INTERACTION).unwrap(), None);
        assert_eq!(stored.previous_assistant.as_deref(), Some(GSA));

        // The next boot finds it cleared: nothing written, nothing forgotten.
        let writes = android.writes;
        let (_, again) = apply(&mut android, stored, &mut forget).unwrap();
        assert_eq!(android.writes, writes);
        assert_eq!(again.previous_assistant.as_deref(), Some(GSA));

        // Off brings the assistant back.
        let off = Preference {
            blocked: false,
            ..again
        };
        let (blocked, _) = apply(&mut android, off, &mut forget).unwrap();
        assert!(!blocked);
        assert_eq!(android.get(ASSISTANT).unwrap().as_deref(), Some(GSA));
        assert_eq!(
            android.get(VOICE_INTERACTION).unwrap().as_deref(),
            Some(GSA)
        );
    }

    #[test]
    fn off_leaves_an_assistant_the_user_picked_alone() {
        let mut android = with_assistant();
        let off = Preference {
            blocked: false,
            previous_assistant: Some("other/assistant".into()),
            previous_voice_interaction: None,
        };
        apply(&mut android, off, &mut forget).unwrap();
        assert_eq!(android.writes, 0);
    }

    #[test]
    fn a_clear_that_fails_halfway_still_remembers_the_assistant() {
        let mut android = with_assistant();
        android.failing_write = Some(2);
        let mut stored = None;
        let result = apply(&mut android, Preference::default(), &mut |preference| {
            stored = Some(preference.clone());
            Ok(())
        });
        assert!(result.is_err());
        // The first write went through: Android has no assistant now.
        assert_eq!(android.get(ASSISTANT).unwrap(), None);
        let stored = stored.expect("the assistant is stored before it is cleared");
        assert_eq!(stored.previous_assistant.as_deref(), Some(GSA));

        // So off still brings it back.
        android.failing_write = None;
        let off = Preference {
            blocked: false,
            ..stored
        };
        let (blocked, _) = apply(&mut android, off, &mut forget).unwrap();
        assert!(!blocked);
        assert_eq!(android.get(ASSISTANT).unwrap().as_deref(), Some(GSA));
        assert_eq!(
            android.get(VOICE_INTERACTION).unwrap().as_deref(),
            Some(GSA)
        );
    }

    /// Live: `cargo test live_super_key -- --ignored` with Android booted.
    /// Leaves the Super key blocked, the assistant it had remembered.
    #[test]
    #[ignore]
    fn live_super_key() {
        let mut android = Adb { active: true };
        assert!(!set(false).unwrap());
        assert!(android.get(ASSISTANT).unwrap().is_some(), "assistant back");
        assert!(set(true).unwrap());
        assert_eq!(android.get(ASSISTANT).unwrap(), None);
        assert_eq!(android.get(VOICE_INTERACTION).unwrap(), None);
        assert!(sync().unwrap());
    }

    #[test]
    fn null_and_empty_mean_no_assistant() {
        assert_eq!(setting_value("null\n"), None);
        assert_eq!(setting_value(""), None);
        assert_eq!(setting_value(&format!("{GSA}\r\n")).as_deref(), Some(GSA));
    }
}
