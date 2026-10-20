//! Android shell access through ADB.
//!
//! `waydroid shell` requires root (`actionNeedRoot` in waydroid 1.6.3
//! `tools/__init__.py`), but `waydroid adb connect` does not: it only needs the
//! host `adb` and points it at the container IP. App controls therefore go
//! through `adb shell` instead of a polkit prompt per action.

use crate::error::{AppError, Result};
use crate::resolve_program;
use crate::waydroid::{self, Waydroid, TIMEOUT_QUICK};

const HINT: &str = "Install adb (android-tools) to run Android commands, or use root access.";

/// Run one arbitrary device shell line. That is the point of `adb_cmd`: the
/// caller explicitly asks for a shell command inside Android.
pub fn shell_line(line: &str) -> Result<String> {
    shell(&[line])
}

/// Run `args` inside Android: `waydroid adb connect`, then `adb shell ...`.
///
/// ponytail: reconnects on every call. Cache the connection state if startup
/// latency ever shows up in the UI; adb keeps the device connected anyway.
pub fn shell(args: &[&str]) -> Result<String> {
    // Resolve adb first: `waydroid adb connect` itself fails with a less useful
    // "Could not find adb" when it is missing.
    let adb = resolve_program("adb").ok_or_else(|| AppError::not_found("adb", HINT))?;
    Waydroid::detect()?.adb_connect()?;

    let mut argv = vec!["shell".to_string()];
    argv.extend(args.iter().map(|arg| (*arg).to_string()));
    waydroid::run_program(&adb, &argv, TIMEOUT_QUICK, false)
}

/// Best effort "Android finished booting" probe.
pub fn booted() -> bool {
    shell(&["getprop", "sys.boot_completed"]).is_ok_and(|output| output.trim() == "1")
}

/// Android package names are letters, digits, `.` and `_`. Everything else is
/// rejected before it can reach `am force-stop` as an argv word, because adb
/// joins the words into one device shell line.
pub fn is_valid_package(package: &str) -> bool {
    !package.is_empty()
        && package
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_names_are_checked_before_they_reach_the_shell() {
        assert!(is_valid_package("org.fdroid.fdroid"));
        assert!(is_valid_package("com.android.vending"));
        assert!(!is_valid_package(""));
        assert!(!is_valid_package("org.fdroid.fdroid; rm -rf /"));
        assert!(!is_valid_package("com.foo bar"));
        assert!(!is_valid_package("com.foo\nam start"));
    }
}
