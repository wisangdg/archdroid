//! Device profile: LDPlayer-style per-device settings mapped onto the knobs
//! Waydroid actually has (verified against waydroid 1.6.3).
//! - resolution/DPI -> `wm size` / `wm density` through adb (runtime, survives
//!   reboots because Android stores it as a display override)
//! - manufacturer/model/locale -> `[properties]` in `/var/lib/waydroid/waydroid.cfg`,
//!   which `tools/helpers/lxc.py` turns into `waydroid_base.prop` on the next
//!   container start (so a restart is required)
//! - CPU/RAM/IMEI/IMSI/MAC/Android ID/phone/root have no Waydroid knob at all;
//!   [`FIELDS`] marks them unsupported instead of failing silently.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};

pub const WAYDROID_CFG: &str = "/var/lib/waydroid/waydroid.cfg";

/// The file `make_prop()` copies verbatim into `/vendor/waydroid.prop`, which is
/// what `resetprop` feeds Android at boot. `lxc.py` folds `[properties]` from
/// `WAYDROID_CFG` into this file only while the container is being set up, so a
/// plain service restart can leave an identity change unapplied.
pub const WAYDROID_BASE_PROP: &str = "/var/lib/waydroid/waydroid_base.prop";
const APP_DIR: &str = "io.github.archdroid"; // matches `identifier` in tauri.conf.json
const PROFILE_FILE: &str = "profile.json";

/// Everything the profile editor can store. `0`/empty means "leave the system
/// default alone"; nothing is invented for fields Waydroid does not expose.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct DeviceProfile {
    pub width: u32,
    pub height: u32,
    pub dpi: u32,
    pub cpu: u32,
    pub memory: u32,
    pub root: bool,
    pub mac: String,
    pub imei: String,
    pub imsi: String,
    pub androidid: String,
    pub manufacturer: String,
    pub model: String,
    pub phone: String,
    pub locale: String,
}

#[derive(Serialize)]
pub struct FieldSupport {
    pub field: &'static str,
    pub supported: bool,
    pub detail: &'static str,
}

/// The mapping table the UI reads to hide fields Waydroid cannot honour.
pub const FIELDS: &[FieldSupport] = &[
    FieldSupport {
        field: "width",
        supported: true,
        detail: "adb shell wm size <w>x<h> (immediate)",
    },
    FieldSupport {
        field: "height",
        supported: true,
        detail: "adb shell wm size <w>x<h> (immediate)",
    },
    FieldSupport {
        field: "dpi",
        supported: true,
        detail: "adb shell wm density <dpi> (immediate)",
    },
    FieldSupport {
        field: "cpu",
        supported: false,
        detail: "Waydroid is an LXC container; it uses the host CPU, no per-device count.",
    },
    FieldSupport {
        field: "memory",
        supported: false,
        detail: "The container shares host RAM; there is no memory size knob.",
    },
    FieldSupport {
        field: "root",
        supported: false,
        detail: "waydroid shell always runs as root; there is no toggle.",
    },
    FieldSupport {
        field: "mac",
        supported: false,
        detail: "The veth MAC is created by waydroid-net.sh; not configurable yet.",
    },
    FieldSupport {
        field: "imei",
        supported: false,
        detail: "No radio stack, so there is no IMEI to set.",
    },
    FieldSupport {
        field: "imsi",
        supported: false,
        detail: "No SIM/radio stack, so there is no IMSI to set.",
    },
    FieldSupport {
        field: "androidid",
        supported: false,
        detail:
            "Android 8+ scopes ANDROID_ID per app signing key; a single override is not effective.",
    },
    FieldSupport {
        field: "manufacturer",
        supported: true,
        detail: "ro.product.manufacturer in waydroid.cfg + waydroid_base.prop (needs a container restart)",
    },
    FieldSupport {
        field: "model",
        supported: true,
        detail: "ro.product.model in waydroid.cfg + waydroid_base.prop (needs a container restart)",
    },
    FieldSupport {
        field: "phone",
        supported: false,
        detail: "No telephony service in Waydroid.",
    },
    FieldSupport {
        field: "locale",
        supported: true,
        detail: "persist.sys.locale in waydroid.cfg + waydroid_base.prop (needs a container restart)",
    },
];

pub fn unsupported_fields() -> Vec<&'static str> {
    FIELDS
        .iter()
        .filter(|field| !field.supported)
        .map(|field| field.field)
        .collect()
}

#[derive(Serialize, Default)]
pub struct ApplyReport {
    /// Applied immediately through adb.
    pub applied: Vec<&'static str>,
    /// Written to waydroid.cfg but waiting for the privileged write + restart.
    pub pending: Vec<&'static str>,
    /// Could not be applied now (session stopped or adb missing).
    pub deferred: Vec<&'static str>,
    pub unsupported: Vec<&'static str>,
    pub restart_required: bool,
    pub notes: Vec<String>,
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_DIR)
        .join(PROFILE_FILE)
}

/// A missing or corrupt file is not an error: the profile is a convenience
/// layer, and the UI should open on defaults instead of a crashed command.
pub fn load_from(path: &Path) -> DeviceProfile {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save_to(path: &Path, profile: &DeviceProfile) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(profile).map_err(io::Error::other)?;
    fs::write(path, json)
}

/// Reject values that would break `wm` or smuggle directives into the INI file.
pub fn validate(profile: &DeviceProfile) -> Result<(), String> {
    if (profile.width == 0) != (profile.height == 0) {
        return Err("width and height must be set together".to_string());
    }
    if profile.width > 7680 || profile.height > 4320 {
        return Err("resolution is out of range (max 7680x4320)".to_string());
    }
    if profile.dpi != 0 && !(72..=1000).contains(&profile.dpi) {
        return Err("dpi must be 0 or between 72 and 1000".to_string());
    }

    for (field, value, max) in [
        ("manufacturer", &profile.manufacturer, 64),
        ("model", &profile.model, 64),
        ("locale", &profile.locale, 32),
    ] {
        if value.chars().any(char::is_control) {
            return Err(format!("{field} must not contain control characters"));
        }
        if value.len() > max {
            return Err(format!("{field} is too long (max {max} bytes)"));
        }
    }
    if !profile.locale.is_empty()
        && !profile
            .locale
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err("locale must look like en-US or id-ID".to_string());
    }
    Ok(())
}

/// Profile field -> `[properties]` key. Empty values remove the override.
pub fn property_updates(profile: &DeviceProfile) -> Vec<(&'static str, Option<String>)> {
    fn value(text: &str) -> Option<String> {
        (!text.is_empty()).then(|| text.to_string())
    }
    vec![
        ("ro.product.manufacturer", value(&profile.manufacturer)),
        ("ro.product.model", value(&profile.model)),
        ("persist.sys.locale", value(&profile.locale)),
    ]
}

fn split_key_value(line: &str) -> Option<(&str, &str)> {
    line.split_once('=')
        .or_else(|| line.split_once(':'))
        .map(|(key, value)| (key.trim(), value.trim()))
}

/// Which profile fields differ from the values currently in waydroid.cfg.
pub fn changed_fields(
    current: &HashMap<String, String>,
    updates: &[(&'static str, Option<String>)],
) -> Vec<&'static str> {
    let field = |key: &str| match key {
        "ro.product.manufacturer" => "manufacturer",
        "ro.product.model" => "model",
        "persist.sys.locale" => "locale",
        _ => "unknown",
    };
    updates
        .iter()
        .filter_map(|(key, value)| {
            let current = current.get(*key).map(String::as_str).unwrap_or_default();
            (current != value.as_deref().unwrap_or_default()).then(|| field(key))
        })
        .collect()
}

pub fn read_properties(cfg: &str) -> HashMap<String, String> {
    let mut properties = HashMap::new();
    let mut in_properties = false;
    for line in cfg.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_properties = trimmed == "[properties]";
            continue;
        }
        if in_properties {
            if let Some((key, value)) = split_key_value(trimmed) {
                properties.insert(key.to_string(), value.to_string());
            }
        }
    }
    properties
}

/// Rewrite only the managed keys of `[properties]`, keeping every other line
/// untouched (the configparser Waydroid reads does the same thing on save).
/// Applying the same updates twice returns the same text.
pub fn apply_properties(cfg: &str, updates: &[(&str, Option<String>)]) -> String {
    let append_updates = |out: &mut String| {
        for (key, value) in updates {
            if let Some(value) = value {
                out.push_str(&format!("{key}={value}\n"));
            }
        }
    };

    let mut out = String::new();
    let mut in_properties = false;
    let mut saw_properties = false;
    let mut inserted = false;

    for line in cfg.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if in_properties && !inserted {
                append_updates(&mut out);
                inserted = true;
            }
            in_properties = trimmed == "[properties]";
            saw_properties |= in_properties;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if in_properties {
            if let Some((key, _)) = split_key_value(trimmed) {
                if updates.iter().any(|(managed, _)| *managed == key) {
                    continue;
                }
            }
        }
        out.push_str(line);
        out.push('\n');
    }

    if in_properties && !inserted {
        append_updates(&mut out);
    }
    if !saw_properties {
        out.push_str("[properties]\n");
        append_updates(&mut out);
    }
    out
}

/// Root-side script that replaces waydroid.cfg atomically. The payload is
/// base64 so no profile value can escape its quoting.
pub fn cfg_write_script(cfg: &str) -> String {
    let encoded = general_purpose::STANDARD.encode(cfg.as_bytes());
    format!(
        "set -euo pipefail\nprintf '%s' '{encoded}' | base64 -d > '{WAYDROID_CFG}.tmp'\nchmod 644 '{WAYDROID_CFG}.tmp'\nmv '{WAYDROID_CFG}.tmp' '{WAYDROID_CFG}'\n"
    )
}

/// Root-side script that replaces waydroid_base.prop atomically.
pub fn base_prop_write_script(propfile: &str) -> String {
    let encoded = general_purpose::STANDARD.encode(propfile.as_bytes());
    format!(
        "set -euo pipefail\nprintf '%s' '{encoded}' | base64 -d > '{WAYDROID_BASE_PROP}.tmp'\nchmod 644 '{WAYDROID_BASE_PROP}.tmp'\nmv '{WAYDROID_BASE_PROP}.tmp' '{WAYDROID_BASE_PROP}'\n"
    )
}

/// The keys of a plain `key=value` prop file (no INI sections).
pub fn read_propfile(propfile: &str) -> HashMap<String, String> {
    let mut properties = HashMap::new();
    for line in propfile.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = split_key_value(trimmed) {
            properties.insert(key.to_string(), value.to_string());
        }
    }
    properties
}

/// Rewrite only the managed keys of a plain prop file, keeping every other line
/// untouched. Applying the same updates twice returns the same text.
pub fn apply_propfile(propfile: &str, updates: &[(&str, Option<String>)]) -> String {
    let managed = |key: &str| updates.iter().any(|(managed, _)| *managed == key);

    let mut out = String::new();
    for line in propfile.lines() {
        let key = split_key_value(line.trim())
            .map(|(key, _)| key)
            .unwrap_or_default();
        if !key.is_empty() && managed(key) {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    for (key, value) in updates {
        if let Some(value) = value {
            out.push_str(&format!("{key}={value}\n"));
        }
    }
    out
}

/// Identity keys Android is using right now, in the order they were requested.
///
/// One value per line, because `getprop a; getprop b` prints exactly that. Blank
/// lines are dropped so an unset property cannot shift the remaining values.
pub fn parse_getprop_values(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

/// Overlay the values actually stored in waydroid.cfg (what the next boot will
/// use). A managed key that is absent means "system default", not "keep the
/// saved JSON value" — that is what makes `profile_get` report effective state
/// after a write was declined.
pub fn overlay_cfg(profile: &mut DeviceProfile, cfg: &str) {
    let properties = read_properties(cfg);
    for (key, field) in [
        ("ro.product.manufacturer", &mut profile.manufacturer),
        ("ro.product.model", &mut profile.model),
        ("persist.sys.locale", &mut profile.locale),
    ] {
        *field = properties.get(key).cloned().unwrap_or_default();
    }
}

/// `wm size` prints physical and (when overridden) override size. The override
/// is what Android currently uses, so it wins.
pub fn parse_wm_size(output: &str) -> Option<(u32, u32)> {
    let mut physical = None;
    let mut override_size = None;
    for line in output.lines() {
        let Some((label, value)) = line.split_once(':') else {
            continue;
        };
        let Some((width, height)) = value.trim().split_once('x') else {
            continue;
        };
        let Ok(width) = width.trim().parse::<u32>() else {
            continue;
        };
        let Ok(height) = height.trim().parse::<u32>() else {
            continue;
        };
        match label.trim().to_ascii_lowercase().as_str() {
            "physical size" => physical = Some((width, height)),
            "override size" => override_size = Some((width, height)),
            _ => {}
        }
    }
    override_size.or(physical)
}

pub fn parse_wm_density(output: &str) -> Option<u32> {
    let mut physical = None;
    let mut override_density = None;
    for line in output.lines() {
        let Some((label, value)) = line.split_once(':') else {
            continue;
        };
        let Ok(value) = value.trim().parse::<u32>() else {
            continue;
        };
        match label.trim().to_ascii_lowercase().as_str() {
            "physical density" => physical = Some(value),
            "override density" => override_density = Some(value),
            _ => {}
        }
    }
    override_density.or(physical)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "archdroid-profile-{}-{name}.json",
            std::process::id()
        ))
    }

    fn sample_profile() -> DeviceProfile {
        DeviceProfile {
            width: 720,
            height: 1280,
            dpi: 160,
            manufacturer: "Acme".to_string(),
            model: "Droid X".to_string(),
            locale: "id-ID".to_string(),
            ..DeviceProfile::default()
        }
    }

    #[test]
    fn profile_round_trips_through_json() {
        let path = temp_path("roundtrip");
        let profile = sample_profile();
        save_to(&path, &profile).expect("save");
        assert_eq!(load_from(&path), profile);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn missing_or_corrupt_file_falls_back_to_default() {
        let path = temp_path("corrupt");
        let _ = fs::remove_file(&path);
        assert_eq!(load_from(&path), DeviceProfile::default());

        fs::write(&path, "{not json").expect("write corrupt file");
        assert_eq!(load_from(&path), DeviceProfile::default());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn apply_properties_is_idempotent_and_keeps_other_keys() {
        let cfg =
            "[waydroid]\narch = arm64\n\n[properties]\nro.product.model = Old\nro.foo = bar\n";
        let updates = vec![
            ("ro.product.manufacturer", Some("Acme".to_string())),
            ("ro.product.model", None),
            ("persist.sys.locale", Some("id-ID".to_string())),
        ];

        let once = apply_properties(cfg, &updates);
        let twice = apply_properties(&once, &updates);
        assert_eq!(once, twice);
        assert!(once.contains("arch = arm64"));
        assert!(once.contains("ro.foo = bar"));
        assert!(once.contains("ro.product.manufacturer=Acme"));
        assert!(once.contains("persist.sys.locale=id-ID"));
        assert!(!once.contains("ro.product.model"));
    }

    #[test]
    fn apply_properties_creates_the_section_when_missing() {
        let once = apply_properties(
            "[waydroid]\narch=arm64\n",
            &[("ro.product.model", Some("Droid X".to_string()))],
        );
        assert!(once.starts_with("[waydroid]\narch=arm64\n[properties]\n"));
        assert!(once.contains("ro.product.model=Droid X"));
        assert_eq!(
            read_properties(&once)
                .get("ro.product.model")
                .map(String::as_str),
            Some("Droid X")
        );
    }

    #[test]
    fn read_properties_only_reads_the_properties_section() {
        let cfg = "[waydroid]\narch=arm64\n[properties]\nro.product.model=Acme\npersist.sys.locale=id-ID\n";
        let properties = read_properties(cfg);
        assert_eq!(properties.get("arch"), None);
        assert_eq!(properties.get("ro.product.model").unwrap(), "Acme");
        assert_eq!(properties.get("persist.sys.locale").unwrap(), "id-ID");
    }

    #[test]
    fn cfg_write_script_only_contains_base64_payload() {
        let cfg = "[properties]\nro.product.model=Droid X\npersist.sys.locale=id-ID\n";
        let script = cfg_write_script(cfg);
        assert!(script.starts_with("set -euo pipefail\n"));
        let encoded = script
            .split_once("printf '%s' '")
            .and_then(|(_, rest)| rest.split_once('\''))
            .map(|(payload, _)| payload)
            .expect("quoted payload");
        let decoded = general_purpose::STANDARD.decode(encoded).expect("base64");
        assert_eq!(String::from_utf8(decoded).unwrap(), cfg);
        assert!(!encoded.contains('\''));
    }

    #[test]
    fn apply_propfile_is_idempotent_and_replaces_managed_keys() {
        let propfile =
            "ro.hardware.egl=mesa\nro.product.model=Old\nro.product.cpu.abilist=x86_64\n";
        let updates = vec![
            ("ro.product.manufacturer", Some("Acme".to_string())),
            ("ro.product.model", Some("Droid X".to_string())),
            ("persist.sys.locale", None),
        ];

        let once = apply_propfile(propfile, &updates);
        let twice = apply_propfile(&once, &updates);
        assert_eq!(once, twice);
        assert!(once.contains("ro.hardware.egl=mesa"));
        assert!(once.contains("ro.product.cpu.abilist=x86_64"));
        assert!(!once.contains("ro.product.model=Old"));
        assert!(once.contains("ro.product.model=Droid X"));
        assert!(once.contains("ro.product.manufacturer=Acme"));
        assert_eq!(
            read_propfile(&once)
                .get("ro.product.model")
                .map(String::as_str),
            Some("Droid X")
        );
    }

    #[test]
    fn apply_propfile_removes_a_key_when_the_value_is_cleared() {
        let propfile = "ro.hardware.egl=mesa\npersist.sys.locale=id-ID\n";
        let cleared = apply_propfile(propfile, &[("persist.sys.locale", None)]);
        assert!(!cleared.contains("persist.sys.locale"));
        assert!(cleared.contains("ro.hardware.egl=mesa"));
    }

    #[test]
    fn read_propfile_skips_comments_and_blank_lines() {
        let propfile = "# comment\n\nro.product.model=Droid X\n  ro.foo=bar  \n";
        let properties = read_propfile(propfile);
        assert_eq!(properties.get("# comment"), None);
        assert_eq!(properties.get("ro.product.model").unwrap(), "Droid X");
        assert_eq!(properties.get("ro.foo").unwrap(), "bar");
    }

    #[test]
    fn base_prop_write_script_only_contains_base64_payload() {
        let propfile = "ro.product.model=Droid X\n";
        let script = base_prop_write_script(propfile);
        let encoded = script
            .split_once("printf '%s' '")
            .and_then(|(_, rest)| rest.split_once('\''))
            .map(|(payload, _)| payload)
            .expect("quoted payload");
        let decoded = general_purpose::STANDARD.decode(encoded).expect("base64");
        assert_eq!(String::from_utf8(decoded).unwrap(), propfile);
        assert!(script.contains(WAYDROID_BASE_PROP));
        assert!(!encoded.contains('\''));
    }

    #[test]
    fn parse_getprop_values_drops_blank_lines() {
        assert_eq!(
            parse_getprop_values("Pixel 7\nGoogle\n\n"),
            vec!["Pixel 7".to_string(), "Google".to_string()]
        );
        assert!(parse_getprop_values("").is_empty());
    }

    #[test]
    fn cfg_values_round_trip_through_the_profile() {
        let profile = sample_profile();
        let cfg = apply_properties("", &property_updates(&profile));
        let mut loaded = profile.clone();
        overlay_cfg(&mut loaded, &cfg);
        assert_eq!(loaded, profile);
    }

    #[test]
    fn overlay_cfg_clears_identity_fields_missing_from_disk() {
        let mut profile = sample_profile();
        overlay_cfg(&mut profile, "[properties]\nro.product.model=Droid Y\n");
        assert_eq!(profile.model, "Droid Y");
        assert_eq!(profile.manufacturer, "");
        assert_eq!(profile.locale, "");
    }

    #[test]
    fn wm_output_prefers_the_override() {
        let output = "Physical size: 1080x1920\nOverride size: 720x1280\nPhysical density: 420\nOverride density: 160\n";
        assert_eq!(parse_wm_size(output), Some((720, 1280)));
        assert_eq!(parse_wm_density(output), Some(160));

        let physical_only = "Physical size: 1080x1920\nPhysical density: 420\n";
        assert_eq!(parse_wm_size(physical_only), Some((1080, 1920)));
        assert_eq!(parse_wm_density(physical_only), Some(420));
    }

    #[test]
    fn validate_rejects_unsafe_or_impossible_values() {
        assert!(validate(&sample_profile()).is_ok());

        let mut only_width = sample_profile();
        only_width.height = 0;
        assert!(validate(&only_width).is_err());

        let mut injected = sample_profile();
        injected.manufacturer = "Acme\n[waydroid]\nimages_path=/tmp".to_string();
        assert!(validate(&injected).is_err());

        let mut bad_locale = sample_profile();
        bad_locale.locale = "en US".to_string();
        assert!(validate(&bad_locale).is_err());

        let mut bad_dpi = sample_profile();
        bad_dpi.dpi = 20;
        assert!(validate(&bad_dpi).is_err());
    }

    #[test]
    fn changed_fields_reports_only_real_differences() {
        let mut current = HashMap::new();
        current.insert("ro.product.manufacturer".to_string(), "Acme".to_string());
        current.insert("persist.sys.locale".to_string(), "en-US".to_string());
        let updates = vec![
            ("ro.product.manufacturer", Some("Acme".to_string())),
            ("ro.product.model", Some("Droid X".to_string())),
            ("persist.sys.locale", None),
        ];
        assert_eq!(changed_fields(&current, &updates), vec!["model", "locale"]);
    }

    #[test]
    fn the_mapping_table_marks_the_unmapped_fields() {
        let unsupported = unsupported_fields();
        for field in [
            "cpu",
            "memory",
            "root",
            "mac",
            "imei",
            "imsi",
            "androidid",
            "phone",
        ] {
            assert!(
                unsupported.contains(&field),
                "{field} should be unsupported"
            );
        }
        for field in ["width", "height", "dpi", "manufacturer", "model", "locale"] {
            assert!(!unsupported.contains(&field), "{field} should be supported");
        }
    }
}
