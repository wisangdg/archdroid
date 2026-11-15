//! Backend of the keymap editor (Keymapping → Edit on the game).
//!
//! The editor draws controls on a screenshot of the running game, so a button
//! lands where the game shows it. What it saves is a plain Phantom profile,
//! `archdroid-<package>.json` in Phantom's own profile library, so Phantom runs
//! it and its GUI can open it too. Positions are normalized (0..1), and the
//! profile's `screen` is the screenshot's size: the size Android renders at.

use std::path::{Path, PathBuf};

use base64::Engine;
use serde::Serialize;
use serde_json::Value;

use crate::error::{AppError, Result};

/// A saved profile larger than this is not something the editor wrote.
const MAX_PROFILE_BYTES: usize = 256 * 1024;
/// Phantom allows more nodes than touches; this only stops runaway input.
const MAX_NODES: usize = 128;

/// The editor's backdrop.
#[derive(Serialize)]
pub struct Screenshot {
    pub data_url: String,
    pub width: u32,
    pub height: u32,
}

/// Width and height from a PNG's IHDR chunk.
pub fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || &bytes[..8] != SIGNATURE || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let read = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap());
    let (width, height) = (read(16), read(20));
    (width > 0 && height > 0).then_some((width, height))
}

/// Screenshot PNG bytes as the editor's backdrop.
pub fn screenshot_from_png(bytes: &[u8]) -> Result<Screenshot> {
    let (width, height) = png_size(bytes).ok_or_else(|| {
        AppError::command_failed("Android returned something that is not a PNG screenshot.")
    })?;
    Ok(Screenshot {
        data_url: format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ),
        width,
        height,
    })
}

/// Phantom's profile library (`$XDG_CONFIG_HOME/phantom/profiles`).
fn profiles_dir() -> Result<PathBuf> {
    dirs::config_dir()
        .map(|dir| dir.join("phantom/profiles"))
        .ok_or_else(|| AppError::command_failed("Cannot find the config directory (no HOME)."))
}

fn profile_file(dir: &Path, package: &str) -> Result<PathBuf> {
    if !crate::adb::is_valid_package(package) {
        return Err(AppError::invalid_input(format!(
            "Invalid Android package name: {package:?}"
        )));
    }
    Ok(dir.join(format!("archdroid-{package}.json")))
}

/// Where the profile of `package` lives.
pub fn profile_path(package: &str) -> Result<PathBuf> {
    profile_file(&profiles_dir()?, package)
}

/// The saved profile of `package`, or `None` when it has none yet.
pub fn load(package: &str) -> Result<Option<Value>> {
    read_profile(&profile_path(package)?)
}

fn read_profile(path: &Path) -> Result<Option<Value>> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map(Some).map_err(|error| {
            AppError::command_failed(format!(
                "{} is not valid JSON ({error}); fix or delete it.",
                path.display()
            ))
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(AppError::Io(error)),
    }
}

/// Save `profile` for `package`; returns the file. Written to a temporary file
/// and renamed, so Phantom never reads half a profile.
pub fn save(package: &str, profile: &Value) -> Result<PathBuf> {
    write_profile(&profiles_dir()?, package, profile)
}

fn write_profile(dir: &Path, package: &str, profile: &Value) -> Result<PathBuf> {
    validate(profile).map_err(AppError::invalid_input)?;
    let path = profile_file(dir, package)?;
    let text = serde_json::to_string_pretty(profile)
        .map_err(|error| AppError::command_failed(error.to_string()))?;
    if text.len() > MAX_PROFILE_BYTES {
        return Err(AppError::invalid_input("The keymap is too large to save."));
    }
    std::fs::create_dir_all(dir)?;
    let partial = path.with_extension("json.part");
    std::fs::write(&partial, text + "\n")?;
    std::fs::rename(&partial, &path)?;
    Ok(path)
}

/// The shape Phantom needs before it can judge a profile. Phantom validates
/// the details (keys, slots, layers) on load; this keeps the editor from
/// writing something that is not a profile at all.
pub fn validate(profile: &Value) -> std::result::Result<(), String> {
    if profile.get("version").and_then(Value::as_u64) != Some(1) {
        return Err("version must be 1".into());
    }
    let screen = profile.get("screen").ok_or("screen is missing")?;
    for side in ["width", "height"] {
        if !screen
            .get(side)
            .and_then(Value::as_u64)
            .is_some_and(|value| (1..=16384).contains(&value))
        {
            return Err(format!("screen.{side} must be a positive size"));
        }
    }
    let nodes = profile
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or("nodes must be a list")?;
    if nodes.len() > MAX_NODES {
        return Err(format!("at most {MAX_NODES} controls"));
    }
    let mut ids = std::collections::HashSet::new();
    for node in nodes {
        let id = node
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or("every control needs an id")?;
        if !ids.insert(id) {
            return Err(format!("two controls are called {id:?}"));
        }
        node.get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{id} has no type"))?;
        for point in ["pos", "anchor", "start", "end"] {
            if let Some(at) = node.get(point) {
                for axis in ["x", "y"] {
                    let inside = at
                        .get(axis)
                        .and_then(Value::as_f64)
                        .is_some_and(|value| (0.0..=1.0).contains(&value));
                    if !inside {
                        return Err(format!("{id}: {point}.{axis} must be within the screen"));
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend(width.to_be_bytes());
        bytes.extend(height.to_be_bytes());
        bytes
    }

    fn profile() -> Value {
        json!({
            "name": "Game", "version": 1,
            "screen": {"width": 1920, "height": 1080},
            "global_sensitivity": 1.0,
            "nodes": [
                {"id": "jump", "type": "tap", "slot": 0, "pos": {"x": 0.9, "y": 0.8}, "key": "Space"},
                {"id": "move", "type": "joystick", "slot": 1, "pos": {"x": 0.2, "y": 0.7},
                 "radius": 0.07, "keys": {"up": "W", "down": "S", "left": "A", "right": "D"}}
            ]
        })
    }

    #[test]
    fn png_size_reads_the_ihdr_and_rejects_other_data() {
        assert_eq!(png_size(&png_header(1280, 800)), Some((1280, 800)));
        assert_eq!(png_size(b"not a png at all, really not"), None);
        assert_eq!(png_size(&png_header(0, 800)), None);
        let shot = screenshot_from_png(&png_header(2, 3)).unwrap();
        assert!(shot
            .data_url
            .starts_with("data:image/png;base64,iVBORw0KGgo"));
        assert_eq!((shot.width, shot.height), (2, 3));
    }

    #[test]
    fn validate_accepts_an_editor_profile_and_names_what_is_wrong() {
        assert_eq!(validate(&profile()), Ok(()));
        let broken = |edit: fn(&mut Value)| {
            let mut value = profile();
            edit(&mut value);
            validate(&value).unwrap_err()
        };
        assert!(broken(|p| p["version"] = json!(2)).contains("version"));
        assert!(broken(|p| p["screen"]["width"] = json!(0)).contains("screen.width"));
        assert!(broken(|p| p["nodes"][1]["id"] = json!("jump")).contains("two controls"));
        assert!(broken(|p| p["nodes"][0]["pos"]["x"] = json!(1.2)).contains("jump: pos.x"));
        assert!(broken(|p| p["nodes"] = json!({})).contains("nodes"));
    }

    #[test]
    fn profiles_are_per_package_and_round_trip() {
        let dir = std::env::temp_dir().join(format!("archdroid-keymap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = write_profile(&dir, "com.example.game", &profile()).unwrap();
        assert_eq!(path, dir.join("archdroid-com.example.game.json"));
        assert_eq!(read_profile(&path).unwrap(), Some(profile()));
        assert_eq!(read_profile(&dir.join("missing.json")).unwrap(), None);
        // A package name cannot carry a path.
        assert!(write_profile(&dir, "../evil", &profile()).is_err());
        // Nothing invalid is written.
        let mut bad = profile();
        bad["version"] = json!(3);
        assert!(write_profile(&dir, "com.example.other", &bad).is_err());
        assert!(!dir.join("archdroid-com.example.other.json").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
