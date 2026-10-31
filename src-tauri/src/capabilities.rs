//! Hardware features the stock Waydroid vendor image never declares.
//!
//! Why this module exists — every claim below was checked against a shipped
//! artifact, not inferred:
//!
//! - Waydroid's vendor image declares only `android.hardware.faketouch` and
//!   `android.hardware.touchscreen` under `/vendor/etc/permissions`.
//! - Standard Android emulator images ship `android.hardware.touchscreen.multitouch.jazzhand.xml` in
//!   that same directory (read out of its system image), declaring the three
//!   multitouch features.
//! - Plants vs Zombies 2 (`com.ea.game.pvz2_row` 13.5.1) requires
//!   `android.hardware.touchscreen.multitouch` with `required="true"`, while
//!   plain `android.hardware.touchscreen` is explicitly *not* required.
//!
//! Google Play hides apps whose required features the device does not declare,
//! and it words that refusal exactly like an ABI mismatch ("Your device isn't
//! compatible with this version"). That is why the real cause is easy to
//! misdiagnose as an ARM translation problem.
//!
//! Installing the file into the Waydroid overlay makes Android declare the
//! features from the next container start on.

use std::path::PathBuf;

/// Overlay path for the vendor partition. Waydroid mounts `<overlay>/vendor`
/// over `/vendor` (helpers/images.py, `mount_rootfs`).
pub const PERMISSIONS_DIR: &str = "/var/lib/waydroid/overlay/vendor/etc/permissions";

/// Kept identical to standard Android emulator file name so comparisons are straightforward.
pub const PERMISSIONS_FILE: &str = "android.hardware.touchscreen.multitouch.xml";

/// The declaration itself. `touchscreen` and `faketouch` are already declared
/// by the image, so only the missing multitouch features are added.
pub const MULTITOUCH_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<!--
  Multitouch capabilities of the touchscreen Waydroid exposes to Android.
  The stock Waydroid vendor image declares only faketouch and touchscreen;
  Google Play hides any app that requires a feature the device never declares.
-->
<permissions>
    <feature name="android.hardware.touchscreen.multitouch" />
    <feature name="android.hardware.touchscreen.multitouch.distinct" />
    <feature name="android.hardware.touchscreen.multitouch.jazzhand" />
</permissions>
"#;

/// The features [`MULTITOUCH_XML`] adds, named the way `pm list features` prints them.
pub const FEATURES: [&str; 3] = [
    "android.hardware.touchscreen.multitouch",
    "android.hardware.touchscreen.multitouch.distinct",
    "android.hardware.touchscreen.multitouch.jazzhand",
];

pub fn target_path() -> PathBuf {
    PathBuf::from(PERMISSIONS_DIR).join(PERMISSIONS_FILE)
}

pub fn file_installed() -> bool {
    target_path().is_file()
}

/// `pm list features` prints one `feature:<name>` per line.
pub fn parse_features(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| line.trim().strip_prefix("feature:"))
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect()
}

/// Which of [`FEATURES`] a parsed feature list is still missing.
pub fn missing_features(declared: &[String]) -> Vec<&'static str> {
    FEATURES
        .iter()
        .copied()
        .filter(|feature| !declared.iter().any(|name| name == feature))
        .collect()
}

/// Root-side script that installs the file atomically. The payload is base64,
/// like `device::cfg_write_script`, so no value can escape its quoting.
pub fn install_script() -> String {
    use base64::{engine::general_purpose, Engine as _};

    let encoded = general_purpose::STANDARD.encode(MULTITOUCH_XML.as_bytes());
    let target = target_path();
    let target = target.to_string_lossy();
    format!(
        "set -euo pipefail\n\
         mkdir -p '{PERMISSIONS_DIR}'\n\
         printf '%s' '{encoded}' | base64 -d > '{target}.tmp'\n\
         chmod 644 '{target}.tmp'\n\
         chown root:root '{target}.tmp'\n\
         mv '{target}.tmp' '{target}'\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;

    #[test]
    fn parse_features_keeps_only_feature_lines() {
        let output = "feature:android.hardware.faketouch\nfeature:android.hardware.touchscreen\nother:junk\n\n";
        assert_eq!(
            parse_features(output),
            vec![
                "android.hardware.faketouch".to_string(),
                "android.hardware.touchscreen".to_string()
            ]
        );
        assert!(parse_features("").is_empty());
    }

    #[test]
    fn missing_features_reports_what_the_image_does_not_declare() {
        let waydroid_defaults = vec![
            "android.hardware.faketouch".to_string(),
            "android.hardware.touchscreen".to_string(),
        ];
        assert_eq!(
            missing_features(&waydroid_defaults).len(),
            FEATURES.len(),
            "the stock image declares none of them"
        );

        let declared: Vec<String> = FEATURES.iter().map(|name| (*name).to_string()).collect();
        assert!(missing_features(&declared).is_empty());

        let partial = vec![FEATURES[0].to_string()];
        assert_eq!(missing_features(&partial), FEATURES[1..].to_vec());
    }

    /// The XML and the constant list must not drift apart: the status check
    /// compares against `FEATURES`, so a feature missing from the file would be
    /// reported as installed forever.
    #[test]
    fn the_xml_declares_exactly_the_features_the_status_check_expects() {
        for feature in FEATURES {
            assert!(
                MULTITOUCH_XML.contains(&format!("name=\"{feature}\"")),
                "{feature} is missing from the XML"
            );
        }
        assert_eq!(
            MULTITOUCH_XML.matches("<feature ").count(),
            FEATURES.len(),
            "the XML declares a feature the status check does not look for"
        );
    }

    #[test]
    fn install_script_carries_the_xml_as_a_base64_payload() {
        let script = install_script();
        assert!(script.starts_with("set -euo pipefail\n"));
        assert!(script.contains(PERMISSIONS_DIR));
        assert!(script.contains(PERMISSIONS_FILE));

        let encoded = script
            .split_once("printf '%s' '")
            .and_then(|(_, rest)| rest.split_once('\''))
            .map(|(payload, _)| payload)
            .expect("quoted payload");
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .expect("base64 payload");
        assert_eq!(String::from_utf8(decoded).unwrap(), MULTITOUCH_XML);
        assert!(!encoded.contains('\''), "payload must not close the quote");
    }

    /// The script is piped into a root shell, so a syntax error would only show
    /// up as a silent no-op for the user. Check it before shipping.
    #[test]
    fn install_script_is_valid_bash() {
        let output = std::process::Command::new("bash")
            .args(["-n", "-c", &install_script()])
            .output()
            .expect("failed to run bash");
        assert!(
            output.status.success(),
            "generated script is not valid bash: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
