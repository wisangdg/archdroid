//! Ad blocking: a hosts file that sends known ad servers nowhere.
//!
//! Android resolves names through `/system/etc/hosts` before DNS, for every
//! app. Waydroid mounts `<overlay>/system` over the system image (`/` is an
//! overlayfs whose top lower layer is `/var/lib/waydroid/overlay`), so a hosts
//! file there replaces Android's from the next container start on. Nothing in
//! the Android image changes, and deleting the file undoes it.
//!
//! The domains are the AdAway default list (mobile ad and tracking SDKs:
//! AppLovin, ironSource/LevelPlay, Unity Ads, Facebook Audience Network,
//! AdMob, ...), vendored in `resources/adaway-hosts.txt`. Nothing is
//! downloaded, and no DNS traffic goes to a third party.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::elevate::sh_quote;

/// AdAway default blocklist, commit 7b5368f0 of
/// <https://github.com/AdAway/adaway.github.io> (CC BY 3.0).
const ADAWAY_HOSTS: &str = include_str!("../resources/adaway-hosts.txt");

/// First line of a hosts file ArchDroid wrote; anything else is left alone.
pub const MARKER: &str = "# ArchDroid ad blocking";

/// Waydroid's overlay for `/system/etc` (helpers/images.py, `mount_rootfs`).
pub const OVERLAY_DIR: &str = "/var/lib/waydroid/overlay/system/etc";

/// Where Android reads it.
pub const ANDROID_HOSTS: &str = "/system/etc/hosts";

pub fn overlay_hosts() -> PathBuf {
    Path::new(OVERLAY_DIR).join("hosts")
}

/// A name a hosts file can carry and a script can pass through unquoted.
fn valid_domain(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 253
        && name.contains('.')
        && !name.starts_with(['.', '-'])
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b".-".contains(&byte))
}

/// The domains of a hosts list, once each, in list order.
pub fn domains(list: &str) -> Vec<&str> {
    let mut seen = HashSet::new();
    list.lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            match (fields.next(), fields.next()) {
                (Some("127.0.0.1" | "0.0.0.0"), Some(name)) => Some(name),
                _ => None,
            }
        })
        .filter(|name| valid_domain(name) && seen.insert(*name))
        .collect()
}

/// The `0.0.0.0 <domain>` lines the install script copies into the hosts
/// file. `0.0.0.0` fails a connection at once instead of trying a local port.
pub fn blocklist() -> String {
    domains(ADAWAY_HOSTS)
        .into_iter()
        .map(|name| format!("0.0.0.0 {name}\n"))
        .collect()
}

/// Whether a hosts file's text is one ArchDroid wrote.
pub fn is_managed(text: &str) -> bool {
    text.lines().next() == Some(MARKER)
}

/// What the overlay holds right now.
#[derive(Debug, PartialEq)]
pub enum Overlay {
    Absent,
    Managed,
    /// A hosts file someone else put there.
    Foreign,
}

pub fn overlay_state() -> Overlay {
    match std::fs::read_to_string(overlay_hosts()) {
        Ok(text) if is_managed(&text) => Overlay::Managed,
        Ok(_) => Overlay::Foreign,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Overlay::Absent,
        // Unreadable but present: treat it as not ours.
        Err(_) => Overlay::Foreign,
    }
}

const FOREIGN_CHECK: &str = "if [ -e \"$target\" ] && [ \"$(head -n 1 \"$target\")\" != \"$marker\" ]; then\n  \
     echo \"[ERROR] $target was not written by ArchDroid, so it is left alone. Move it away to let ArchDroid manage ad blocking.\" >&2\n  \
     exit 1\nfi\n";

/// Root-side script that installs the hosts file from `staged`, a file the
/// desktop user wrote. The script argument limit (128 KiB) is smaller than the
/// list, hence the file. Root keeps only `0.0.0.0 <domain>` lines from it and
/// writes the header itself, so whatever else that file holds never reaches
/// Android.
pub fn install_script(staged: &Path) -> String {
    let staged = sh_quote(&staged.to_string_lossy());
    let target = overlay_hosts();
    let target = target.to_string_lossy();
    format!(
        "set -euo pipefail\n\
         target='{target}'\n\
         marker='{MARKER}'\n\
         {FOREIGN_CHECK}\
         mkdir -p '{OVERLAY_DIR}'\n\
         {{\n  \
           printf '%s\\n' \"$marker\" '# Written by ArchDroid; turn ad blocking off in ArchDroid to remove it.' '# Blocklist: AdAway default list (https://adaway.org), CC BY 3.0.' '127.0.0.1 localhost' '::1 ip6-localhost'\n  \
           grep -E '^0\\.0\\.0\\.0 [a-z0-9.-]+$' {staged}\n\
         }} > \"$target.tmp\"\n\
         chmod 644 \"$target.tmp\"\n\
         chown root:root \"$target.tmp\"\n\
         mv \"$target.tmp\" \"$target\"\n\
         echo \"[SYSTEM] Blocking $(grep -c '^0\\.0\\.0\\.0 ' \"$target\") ad domains from the next Waydroid start.\"\n"
    )
}

/// Root-side script that removes the hosts file, if ArchDroid wrote it.
pub fn remove_script() -> String {
    let target = overlay_hosts();
    let target = target.to_string_lossy();
    format!(
        "set -euo pipefail\n\
         target='{target}'\n\
         marker='{MARKER}'\n\
         {FOREIGN_CHECK}\
         rm -f \"$target\"\n\
         echo '[SYSTEM] Ad blocking removed; Android uses its own hosts file from the next Waydroid start.'\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domains_keep_blocking_entries_once_and_skip_the_rest() {
        let list = "# comment\n127.0.0.1  localhost\n::1  localhost\n\n\
                    127.0.0.1 ads.example.com\n0.0.0.0 track.example.net # note\n\
                    127.0.0.1 ads.example.com\n10.0.0.1 redirect.example.com\n\
                    127.0.0.1 Bad.Example.com\n127.0.0.1 evil.com;reboot\n127.0.0.1 -x.com\n";
        assert_eq!(domains(list), vec!["ads.example.com", "track.example.net"]);
    }

    #[test]
    fn the_bundled_list_blocks_the_sdks_games_show_ads_through() {
        let blocked = domains(ADAWAY_HOSTS);
        assert!(blocked.len() > 5000, "list shrank to {}", blocked.len());
        for name in [
            // ironSource / LevelPlay, the mediation Plants vs Zombies 2 uses.
            "init.supersonicads.com",
            "outcome-ssp.supersonicads.com",
            // Facebook Audience Network, whose interstitial PvZ2 opened.
            "an.facebook.com",
            "a.applovin.com",
            "pubads.g.doubleclick.net",
        ] {
            assert!(blocked.contains(&name), "{name} is not blocked");
        }
        // Logins and app stores keep working.
        for name in [
            "graph.facebook.com",
            "play.googleapis.com",
            "accounts.google.com",
        ] {
            assert!(!blocked.contains(&name), "{name} must stay reachable");
        }
    }

    #[test]
    fn blocklist_lines_are_what_the_install_script_keeps() {
        let list = blocklist();
        assert!(list.contains("0.0.0.0 an.facebook.com\n"));
        assert!(list
            .lines()
            .all(|line| { line.strip_prefix("0.0.0.0 ").is_some_and(valid_domain) }));
        assert!(is_managed(&format!("{MARKER}\n127.0.0.1 localhost\n")));
        assert!(!is_managed("127.0.0.1 localhost\n"));
    }

    fn bash(script: &str) -> std::process::Output {
        std::process::Command::new("bash")
            .args(["-c", script])
            .output()
            .expect("failed to run bash")
    }

    /// Runs the scripts against a temporary directory instead of the overlay.
    fn retarget(script: &str, dir: &Path) -> String {
        script
            .replace(
                &overlay_hosts().to_string_lossy().to_string(),
                &dir.join("hosts").to_string_lossy(),
            )
            .replace(OVERLAY_DIR, &dir.to_string_lossy())
            .replace("chown root:root \"$target.tmp\"\n", "")
    }

    #[test]
    fn install_keeps_only_block_lines_and_remove_undoes_it() {
        let dir = std::env::temp_dir().join(format!("archdroid-adblock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let staged = dir.join("staged's hosts");
        std::fs::write(
            &staged,
            "0.0.0.0 ads.example.com\n1.2.3.4 play.google.com\n0.0.0.0 x.com; rm -rf /\n",
        )
        .unwrap();
        let target = dir.join("etc/hosts");

        let out = bash(&retarget(&install_script(&staged), &dir.join("etc")));
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let written = std::fs::read_to_string(&target).unwrap();
        assert!(is_managed(&written));
        assert!(written.contains("\n127.0.0.1 localhost\n"));
        assert!(written.ends_with("\n0.0.0.0 ads.example.com\n"));
        assert!(!written.contains("play.google.com") && !written.contains("rm -rf"));

        let out = bash(&retarget(&remove_script(), &dir.join("etc")));
        assert!(out.status.success());
        assert!(!target.exists());
        // Removing twice is fine.
        assert!(bash(&retarget(&remove_script(), &dir.join("etc")))
            .status
            .success());

        // A hosts file someone else wrote is never replaced or deleted.
        std::fs::write(&target, "1.2.3.4 my.lan\n").unwrap();
        for script in [install_script(&staged), remove_script()] {
            let out = bash(&retarget(&script, &dir.join("etc")));
            assert!(!out.status.success());
            assert!(String::from_utf8_lossy(&out.stderr).contains("not written by ArchDroid"));
        }
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "1.2.3.4 my.lan\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scripts_are_valid_bash() {
        for script in [install_script(Path::new("/tmp/x")), remove_script()] {
            let out = std::process::Command::new("bash")
                .args(["-n", "-c", &script])
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }
}
