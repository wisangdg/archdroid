//! Environment for host programs started from inside the AppImage.
//!
//! The AppImage's AppRun points `LD_LIBRARY_PATH`, `GI_TYPELIB_PATH`,
//! `GIO_EXTRA_MODULES`, `PATH`, `XDG_DATA_DIRS`, `PYTHONHOME`, ... into the
//! bundle. ArchDroid itself needs them (WebKit starts its helper processes with
//! them), but a host program must not see them: `/usr/bin/waydroid` loaded the
//! bundled Ubuntu 22.04 glib through `gi` and failed with "undefined symbol:
//! g_once_init_leave_pointer" on an Arch host (0.1.4 AppImage). Every host
//! program is therefore started through [`host_command`], which drops each
//! bundle entry from the inherited environment.

use std::ffi::OsStr;
use std::process::Command;
use std::sync::OnceLock;

/// A variable to change for host programs: `Some(value)` to set, `None` to remove.
type Override = (String, Option<String>);

/// A `Command` for a host program, with the AppImage bundle removed from its
/// environment. Outside an AppImage this is `Command::new`.
pub(crate) fn host_command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    for (key, value) in overrides() {
        match value {
            Some(value) => command.env(key, value),
            None => command.env_remove(key),
        };
    }
    command
}

fn overrides() -> &'static [Override] {
    static OVERRIDES: OnceLock<Vec<Override>> = OnceLock::new();
    OVERRIDES.get_or_init(|| match std::env::var("APPDIR") {
        Ok(dir) => bundle_overrides(&dir, std::env::vars_os()),
        Err(_) => Vec::new(),
    })
}

/// The changes that strip every `appdir` entry from `vars`. Pure, so tests
/// never touch the process environment.
fn bundle_overrides(
    appdir: &str,
    vars: impl Iterator<Item = (std::ffi::OsString, std::ffi::OsString)>,
) -> Vec<Override> {
    let appdir = appdir.trim_end_matches('/');
    if appdir.is_empty() {
        return Vec::new();
    }
    let in_bundle = |entry: &str| {
        entry == appdir
            || entry
                .strip_prefix(appdir)
                .is_some_and(|rest| rest.starts_with('/'))
    };
    vars.filter_map(|(key, value)| {
        let (key, value) = (key.into_string().ok()?, value.into_string().ok()?);
        // Identify the AppImage itself; harmless and useful to host scripts.
        if matches!(key.as_str(), "APPDIR" | "APPIMAGE" | "ARGV0" | "OWD") {
            return None;
        }
        if !value.split(':').any(in_bundle) {
            return None;
        }
        let kept: Vec<&str> = value
            .split(':')
            .filter(|entry| !entry.is_empty() && !in_bundle(entry))
            .collect();
        Some((key, (!kept.is_empty()).then(|| kept.join(":"))))
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn vars(pairs: &[(&str, &str)]) -> impl Iterator<Item = (OsString, OsString)> {
        pairs
            .iter()
            .map(|(k, v)| (OsString::from(k), OsString::from(v)))
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn strips_bundle_entries_seen_in_the_0_1_4_appimage() {
        let dir = "/tmp/.mount_ArchDrMMFDhi";
        let got = bundle_overrides(
            dir,
            vars(&[
                (
                    "LD_LIBRARY_PATH",
                    "/tmp/.mount_ArchDrMMFDhi/usr/lib/:/tmp/.mount_ArchDrMMFDhi/lib64/:/usr/lib/orca-ide/lib:",
                ),
                ("GI_TYPELIB_PATH", "/tmp/.mount_ArchDrMMFDhi//usr/lib/girepository-1.0"),
                ("PATH", "/tmp/.mount_ArchDrMMFDhi/usr/bin/:/usr/local/bin:/usr/bin"),
                (
                    "XDG_DATA_DIRS",
                    "/tmp/.mount_ArchDrMMFDhi/usr/share/:/usr/local/share:/usr/share",
                ),
                ("PYTHONHOME", "/tmp/.mount_ArchDrMMFDhi/usr/"),
                ("GTK_THEME", "Adwaita:dark"),
                ("HOME", "/home/user"),
                ("APPDIR", dir),
                ("APPIMAGE", "/home/user/ArchDroid.AppImage"),
            ]),
        );
        assert_eq!(
            got,
            vec![
                (
                    "LD_LIBRARY_PATH".into(),
                    Some("/usr/lib/orca-ide/lib".into())
                ),
                ("GI_TYPELIB_PATH".into(), None),
                ("PATH".into(), Some("/usr/local/bin:/usr/bin".into())),
                (
                    "XDG_DATA_DIRS".into(),
                    Some("/usr/local/share:/usr/share".into())
                ),
                ("PYTHONHOME".into(), None),
            ]
        );
    }

    #[test]
    fn leaves_lookalike_paths_and_non_appimage_runs_alone() {
        let got = bundle_overrides(
            "/tmp/.mount_abc/",
            vars(&[("PATH", "/tmp/.mount_abcd/bin:/usr/bin")]),
        );
        assert!(got.is_empty());
        assert!(bundle_overrides("", vars(&[("PATH", "/usr/bin")])).is_empty());
    }

    #[test]
    fn host_command_is_plain_outside_an_appimage() {
        // Outside an AppImage (the test process) there is nothing to change.
        let command = host_command("true");
        assert_eq!(command.get_envs().count(), 0);
    }
}
