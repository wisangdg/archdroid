//! Host diagnostics report behind the Settings > Diagnostics panel.
//!
//! [`HostFacts::probe`] reads the system; [`report`] turns those facts into
//! the items the user reads. The decisions (status, wording, remediation) are
//! pure, so every check is tested for each status it can produce.

use std::path::Path;
use std::process::Stdio;

use crate::*;

/// Access to `/dev/kvm`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kvm {
    Ready,
    PermissionDenied,
    Missing,
}

/// Whether the `waydroid` CLI exists, or can be installed from a repository.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WaydroidCli {
    Installed,
    /// No configured repository offers the package.
    NotInRepositories,
    Missing,
}

/// Everything the report is decided from.
#[derive(Clone, Debug)]
pub(crate) struct HostFacts {
    pub pm: Option<platform::PackageManager>,
    pub wayland: bool,
    pub kvm: Kvm,
    pub binder: bool,
    pub gpu: bool,
    pub service_active: bool,
    pub waydroid: WaydroidCli,
    pub adb: bool,
    pub pkexec: bool,
    pub sudo: bool,
    pub polkit_agent: bool,
    /// ArchDroid was started from a terminal, so sudo can ask for a password.
    pub tty: bool,
    pub git: bool,
    pub python: bool,
    pub venv: bool,
    pub unsquashfs: bool,
    pub lzip: bool,
    pub xdg_open: bool,
    pub terminal: Option<String>,
    pub disk_kb: Option<u64>,
    pub online: bool,
    pub initialized: bool,
}

impl HostFacts {
    pub(crate) fn probe() -> Self {
        let pm = platform::PackageManager::detect();
        let kvm_path = Path::new("/dev/kvm");
        let kvm = if !kvm_path.exists() {
            Kvm::Missing
        } else if std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(kvm_path)
            .is_ok()
        {
            Kvm::Ready
        } else {
            Kvm::PermissionDenied
        };
        let waydroid = if has_program("waydroid") {
            WaydroidCli::Installed
        } else if pm
            .is_some_and(|pm| waydroid_availability(pm) == platform::Availability::Unavailable)
        {
            WaydroidCli::NotInRepositories
        } else {
            WaydroidCli::Missing
        };
        let service_active = if let Some(systemctl) =
            resolve_program("systemctl").or_else(|| trusted_program("systemctl"))
        {
            waydroid::run_program(
                &systemctl,
                &["is-active".to_string(), "waydroid-container".to_string()],
                3,
                None,
            )
            .map(|stdout| stdout.trim() == "active")
            .unwrap_or(false)
        } else {
            false
        };
        let (python, venv) = check_python_venv_available();
        Self {
            pm,
            wayland: std::env::var("WAYLAND_DISPLAY").is_ok()
                || std::env::var("XDG_SESSION_TYPE")
                    .map(|s| s.to_lowercase() == "wayland")
                    .unwrap_or(false),
            kvm,
            binder: Path::new("/dev/binderfs").exists()
                || Path::new("/dev/binder").exists()
                || Path::new("/dev/binderfs/binder").exists(),
            gpu: Path::new("/dev/dri/renderD128").exists() || Path::new("/dev/dri").exists(),
            service_active,
            waydroid,
            adb: has_program("adb"),
            pkexec: has_program("pkexec"),
            sudo: has_program("sudo"),
            polkit_agent: polkit_agent_running(),
            tty: std::io::stdin().is_terminal(),
            git: has_program("git"),
            python,
            venv,
            unsquashfs: has_program("unsquashfs"),
            lzip: has_program("lzip"),
            xdg_open: has_program("xdg-open"),
            terminal: find_terminal_emulator(),
            disk_kb: check_available_disk_space_kb(Path::new("/var/lib/waydroid")),
            online: check_network_connectivity(),
            initialized: waydroid::is_initialized(),
        }
    }
}

pub(crate) fn collect_diagnostics() -> DiagnosticsReport {
    report(&HostFacts::probe())
}

fn item(
    key: &str,
    title: &str,
    status: &str,
    value: impl Into<String>,
    detail: impl Into<String>,
    remediation: Option<String>,
) -> DiagnosticItem {
    DiagnosticItem {
        key: key.to_string(),
        title: title.to_string(),
        status: status.to_string(),
        value: value.into(),
        detail: detail.into(),
        remediation,
    }
}

/// The report for `facts`, in the order the UI shows it.
pub(crate) fn report(facts: &HostFacts) -> DiagnosticsReport {
    let pm = facts.pm;
    let install = |packages: &[&str]| pm.map(|p| p.user_install_command(packages));
    let mut items = Vec::new();

    // 1. Session Type (Wayland vs X11)
    items.push(if facts.wayland {
        item("session", "Display Session", "ok", "Wayland Native",
            "Wayland compositor detected. Waydroid runs with native direct rendering.", None)
    } else {
        item("session", "Display Session", "warning", "X11 Display",
            "Waydroid requires Wayland. Under X11, a nested compositor (Weston/cage) may be required.",
            Some("Run inside a Wayland compositor, or use cage: cage -- waydroid show-full-ui".to_string()))
    });

    // 2. KVM Virtualization Acceleration
    items.push(match facts.kvm {
        Kvm::Ready => item(
            "kvm",
            "KVM Acceleration",
            "ok",
            "Hardware KVM Ready",
            "/dev/kvm is available with read/write access.",
            None,
        ),
        Kvm::PermissionDenied => item(
            "kvm",
            "KVM Acceleration",
            "warning",
            "Permission Denied",
            "KVM device exists but user lacks write permission.",
            Some("sudo usermod -aG kvm $USER".to_string()),
        ),
        Kvm::Missing => item(
            "kvm",
            "KVM Acceleration",
            "warning",
            "Not Detected",
            "/dev/kvm not found. Ensure Intel VT-x or AMD-V is enabled in motherboard BIOS.",
            Some("Enable VT-x/AMD-V in BIOS and load kernel module: sudo modprobe kvm".to_string()),
        ),
    });

    // 3. Android Binder IPC
    items.push(if facts.binder {
        item(
            "binder",
            "Android Binder IPC",
            "ok",
            "Binder Nodes Active",
            "Android kernel inter-process communication drivers are active.",
            None,
        )
    } else {
        item(
            "binder",
            "Android Binder IPC",
            "ok",
            "Managed by Daemon",
            "Binderfs will be mounted automatically when container daemon starts.",
            None,
        )
    });

    // 4. GPU 3D Passthrough
    items.push(if facts.gpu {
        item(
            "gpu",
            "GPU Acceleration",
            "ok",
            "Direct Rendering (DRI)",
            "Hardware Direct Rendering Manager node is available for 3D graphics.",
            None,
        )
    } else {
        item(
            "gpu",
            "GPU Acceleration",
            "warning",
            "Software Rendering",
            "Hardware DRI node not found. Android graphics will fall back to software rendering.",
            Some(
                "Install graphics drivers supporting DRI (Direct Rendering Infrastructure)."
                    .to_string(),
            ),
        )
    });

    // 5. Waydroid Container Service
    items.push(if facts.service_active {
        item(
            "service",
            "Container Daemon",
            "ok",
            "Service Active",
            "waydroid-container.service is actively running in background.",
            None,
        )
    } else {
        item(
            "service",
            "Container Daemon",
            "info",
            "Service Inactive",
            "Container daemon is inactive. It will start automatically when session starts.",
            None,
        )
    });

    // 6. Waydroid CLI binary
    items.push(match facts.waydroid {
        WaydroidCli::Installed => item(
            "waydroid",
            "Waydroid Package",
            "ok",
            "Installed",
            "Waydroid command-line tool is available in system PATH.",
            None,
        ),
        WaydroidCli::NotInRepositories => item(
            "waydroid",
            "Waydroid Package",
            "error",
            WAYDROID_NOT_IN_REPOS,
            waydroid_unavailable_detail(pm),
            None,
        ),
        WaydroidCli::Missing => item(
            "waydroid",
            "Waydroid Package",
            "error",
            "Not Installed",
            waydroid_missing_detail(pm),
            install(&["waydroid"]),
        ),
    });

    // 7. ADB / android-tools
    items.push(if facts.adb {
        item("adb", "Android Debug Bridge", "ok", "Installed",
            "adb is available for unprivileged container queries and inspection.", None)
    } else {
        item("adb", "Android Debug Bridge", "warning", "Not Installed",
            "adb is missing. App list, install/uninstall, and Android boot detection fall back to slower or unavailable paths.",
            install(&["adb"]))
    });

    // 8. Root Elevation Tool (polkit / sudo)
    items.push(if facts.pkexec && !facts.polkit_agent {
        item("elevation", "Privilege Elevation",
            if facts.sudo && facts.tty { "warning" } else { "error" },
            "No Polkit Agent",
            "pkexec is installed but no polkit authentication agent is running, so the password prompt cannot appear. Start an agent in your session, or launch ArchDroid from a terminal so sudo can ask instead.",
            Some("Start one, e.g. hyprpolkitagent, polkit-gnome-authentication-agent-1, or lxqt-policykit-agent".to_string()))
    } else if !facts.pkexec && facts.sudo && !facts.tty {
        item("elevation", "Privilege Elevation", "error", "sudo Needs a TTY",
            "Only sudo is available and ArchDroid has no terminal to ask for the password. Administrative operations will fail.",
            Some(format!(
                "{}, or start ArchDroid from a terminal",
                install(&["polkit"]).unwrap_or_else(|| "Install polkit and a polkit agent".to_string())
            )))
    } else if facts.pkexec || facts.sudo {
        let tool = if facts.pkexec { "pkexec" } else { "sudo" };
        item("elevation", "Privilege Elevation", "ok",
            if facts.pkexec { "Polkit (pkexec)" } else { "sudo (terminal)" },
            format!("Authentication agent '{tool}' is available for administrative operations."),
            None)
    } else {
        item("elevation", "Privilege Elevation", "error", "Missing",
            "Neither pkexec nor sudo was found. Administrative operations will fail.",
            install(&["polkit"]))
    });

    // 9. Git
    items.push(if facts.git {
        item(
            "git",
            "Git Version Control",
            "ok",
            "Installed",
            "git is available to download pinned third-party helper checkouts.",
            None,
        )
    } else {
        item(
            "git",
            "Git Version Control",
            "warning",
            "Not Installed",
            "git is required by GApps, ARM Translation, and Phantom installers.",
            install(&["git"]),
        )
    });

    // 10. Python 3 & venv
    items.push(match (facts.python, facts.venv) {
        (true, true) => item("python", "Python 3 & venv", "ok", "Ready",
            "Python 3 and venv module are ready for isolated script execution.", None),
        (true, false) => item("python", "Python 3 & venv", "error", "venv Missing",
            "Python 3 is present but python3-venv is missing. Debian/Ubuntu systems require python3-venv.",
            install(&["python3-venv"])),
        _ => item("python", "Python 3 & venv", "error", "Not Installed",
            "Python 3 is required by GApps and ARM Translation installers.",
            install(&["python3", "python3-venv"])),
    });

    // 11. squashfs extraction tool
    items.push(if facts.unsquashfs {
        item(
            "squashfs",
            "SquashFS Tools",
            "ok",
            "unsquashfs Ready",
            "unsquashfs is available to unpack Android filesystem images.",
            None,
        )
    } else {
        item(
            "squashfs",
            "SquashFS Tools",
            "warning",
            "Missing",
            "unsquashfs is required to extract Android images for GApps and ARM Translation.",
            install(&["squashfs"]),
        )
    });

    // 12. lzip
    items.push(if facts.lzip {
        item(
            "lzip",
            "Lzip Decompressor",
            "ok",
            "lzip Ready",
            "lzip is available to extract MindTheGapps and libhoudini archives.",
            None,
        )
    } else {
        item(
            "lzip",
            "Lzip Decompressor",
            "warning",
            "Missing",
            "lzip is required to decompress archives for GApps and ARM Translation.",
            install(&["lzip"]),
        )
    });

    // 13. xdg-open
    items.push(if facts.xdg_open {
        item(
            "xdg_open",
            "File Manager Launcher",
            "ok",
            "xdg-open Ready",
            "xdg-open is available to open shared storage in your default file manager.",
            None,
        )
    } else {
        item(
            "xdg_open",
            "File Manager Launcher",
            "info",
            "Missing",
            "xdg-open is needed to open the Android shared media folder in a file manager.",
            install(&["xdg-utils"]),
        )
    });

    // 14. Terminal Emulator for Phantom
    items.push(match &facts.terminal {
        Some(term) => item("terminal", "Terminal Emulator", "ok", term.clone(),
            format!("Terminal emulator '{term}' detected to run Phantom Keymapper interactive shell."),
            None),
        None => item("terminal", "Terminal Emulator", "info", "Not Detected",
            "A desktop terminal emulator (alacritty, foot, kitty, konsole, gnome-terminal) is needed to launch Phantom Keymapper.",
            install(&["alacritty"])),
    });

    // 15. Disk Space
    items.push(match facts.disk_kb {
        Some(kb) if kb >= 1_500_000 => item(
            "disk",
            "Available Storage",
            "ok",
            format_disk_space(kb),
            "Sufficient disk space available for system images and container operations.",
            None,
        ),
        Some(kb) if kb >= 500_000 => item(
            "disk",
            "Available Storage",
            "warning",
            format_disk_space(kb),
            "Low disk space (< 1.5 GB). Initializing Waydroid requires ~1.2 GB of free space.",
            Some("Free up at least 1.5 GB on host root or /var filesystem.".to_string()),
        ),
        Some(kb) => item(
            "disk",
            "Available Storage",
            "error",
            format_disk_space(kb),
            "Critically low disk space (< 500 MB). Waydroid container operations may fail.",
            Some("Free up disk space immediately on host root or /var filesystem.".to_string()),
        ),
        None => item(
            "disk",
            "Available Storage",
            "info",
            "Unknown",
            "Unable to query available disk space via df.",
            None,
        ),
    });

    // 16. Network Connectivity
    items.push(if facts.online {
        item("network", "Network Connection", "ok", "Connected",
            "Internet connectivity verified. Online downloads (system images, GApps, ARM translation) ready.",
            None)
    } else {
        item("network", "Network Connection", "warning", "Offline",
            "No internet connection detected. Offline host cannot download container images or third-party packages.",
            Some("Connect your Linux host to the internet to download required system images and packages.".to_string()))
    });

    // 17. Waydroid Android image initialization
    items.push(if facts.initialized {
        item("waydroid_image", "Android System Image", "ok", "Initialized",
            "The Waydroid Android image is initialized and ready for sessions and add-ons.", None)
    } else {
        item("waydroid_image", "Android System Image", "warning", "Not Initialized",
            "The Android image has not been initialized. GApps and ARM Translation need an initialized image.",
            Some("Run the Setup Wizard, or run: sudo waydroid init".to_string()))
    });

    DiagnosticsReport {
        items,
        package_manager: pm.map(|p| p.name().to_string()),
    }
}

/// Debian and Ubuntu releases before Waydroid entered their archives (e.g.
/// Ubuntu 24.04) have no `waydroid` package, so the apt command alone fails
/// with "Unable to locate package"; upstream documents its own repository.
/// Value of the `waydroid` item when no configured repository offers the
/// package. The setup screen blocks Initialize on it: installing would fail
/// only after the root prompt.
pub(crate) const WAYDROID_NOT_IN_REPOS: &str = "Not in Repositories";

/// Ask the package manager, without root and with a short deadline, whether
/// any configured repository offers `waydroid`.
pub(crate) fn waydroid_availability(pm: platform::PackageManager) -> platform::Availability {
    let (program, args) = pm.availability_query("waydroid");
    let Some(path) = trusted_program(program) else {
        return platform::Availability::Unknown;
    };
    let Ok(mut child) = crate::hostenv::host_command(path)
        .args(&args)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return platform::Availability::Unknown;
    };
    // Read on a thread so a chatty tool cannot block on a full pipe.
    let mut stdout = child.stdout.take();
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(out) = stdout.as_mut() {
            let _ = std::io::Read::read_to_string(out, &mut text);
        }
        text
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    let code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(50))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let text = reader.join().unwrap_or_default();
    pm.availability_from(code, &text)
}

fn waydroid_unavailable_detail(pm: Option<platform::PackageManager>) -> String {
    let base = "None of the configured repositories offers a waydroid package, so ArchDroid cannot install it.";
    match pm {
        Some(platform::PackageManager::Apt) => format!(
            "{base} Add the official Waydroid repository (https://docs.waydro.id/usage/install-on-desktops), run `sudo apt update`, then Re-check."
        ),
        Some(platform::PackageManager::Zypper) => format!(
            "{base} openSUSE has no official Waydroid package or binder module: install Waydroid from a community OBS repository you trust (see https://en.opensuse.org/Mobile:MoreGuides:Waydroid), then Re-check."
        ),
        Some(platform::PackageManager::Dnf) => format!(
            "{base} Fedora ships it in its own repositories; RHEL, AlmaLinux and Rocky Linux have it in neither the base nor the EPEL repositories. Install Waydroid from a source you trust (for example a Copr repository), then Re-check."
        ),
        _ => format!("{base} Install Waydroid by other means, then Re-check."),
    }
}

fn waydroid_missing_detail(pm: Option<platform::PackageManager>) -> String {
    let base = "Waydroid command is missing. It must be installed before starting or initializing containers.";
    match pm {
        Some(platform::PackageManager::Apt) => format!(
            "{base} If apt cannot find the waydroid package (e.g. Ubuntu 24.04), add the official Waydroid repository first: https://docs.waydro.id/usage/install-on-desktops"
        ),
        _ => base.to_string(),
    }
}

#[cfg(test)]
mod tests {
    fn healthy() -> HostFacts {
        HostFacts {
            pm: Some(crate::platform::PackageManager::Pacman),
            wayland: true,
            kvm: Kvm::Ready,
            binder: true,
            gpu: true,
            service_active: true,
            waydroid: WaydroidCli::Installed,
            adb: true,
            pkexec: true,
            sudo: true,
            polkit_agent: true,
            tty: false,
            git: true,
            python: true,
            venv: true,
            unsquashfs: true,
            lzip: true,
            xdg_open: true,
            terminal: Some("foot".to_string()),
            disk_kb: Some(20_000_000),
            online: true,
            initialized: true,
        }
    }

    /// (status, value, has remediation) of the item `key` for `facts`.
    fn check(facts: &HostFacts, key: &str) -> (String, String, bool) {
        let report = super::report(facts);
        let item = report
            .items
            .iter()
            .find(|i| i.key == key)
            .unwrap_or_else(|| panic!("no {key} item"));
        (
            item.status.clone(),
            item.value.clone(),
            item.remediation.is_some(),
        )
    }

    #[test]
    fn a_healthy_host_reports_every_check_ok_or_info() {
        let report = super::report(&healthy());
        assert_eq!(report.items.len(), 17);
        assert_eq!(report.package_manager.as_deref(), Some("pacman"));
        for item in &report.items {
            assert_eq!(item.status, "ok", "{}: {}", item.key, item.value);
            assert!(item.remediation.is_none(), "{}", item.key);
        }
    }

    #[test]
    fn each_failing_fact_turns_its_own_check_and_says_how_to_fix_it() {
        type Edit = fn(&mut HostFacts);
        let cases: &[(&str, Edit, &str, &str, bool)] = &[
            (
                "session",
                |f| f.wayland = false,
                "warning",
                "X11 Display",
                true,
            ),
            (
                "kvm",
                |f| f.kvm = Kvm::PermissionDenied,
                "warning",
                "Permission Denied",
                true,
            ),
            (
                "kvm",
                |f| f.kvm = Kvm::Missing,
                "warning",
                "Not Detected",
                true,
            ),
            (
                "binder",
                |f| f.binder = false,
                "ok",
                "Managed by Daemon",
                false,
            ),
            (
                "gpu",
                |f| f.gpu = false,
                "warning",
                "Software Rendering",
                true,
            ),
            (
                "service",
                |f| f.service_active = false,
                "info",
                "Service Inactive",
                false,
            ),
            (
                "waydroid",
                |f| f.waydroid = WaydroidCli::Missing,
                "error",
                "Not Installed",
                true,
            ),
            (
                "waydroid",
                |f| f.waydroid = WaydroidCli::NotInRepositories,
                "error",
                "Not in Repositories",
                false,
            ),
            ("adb", |f| f.adb = false, "warning", "Not Installed", true),
            ("git", |f| f.git = false, "warning", "Not Installed", true),
            ("python", |f| f.venv = false, "error", "venv Missing", true),
            (
                "python",
                |f| {
                    f.python = false;
                    f.venv = false
                },
                "error",
                "Not Installed",
                true,
            ),
            (
                "squashfs",
                |f| f.unsquashfs = false,
                "warning",
                "Missing",
                true,
            ),
            ("lzip", |f| f.lzip = false, "warning", "Missing", true),
            ("xdg_open", |f| f.xdg_open = false, "info", "Missing", true),
            (
                "terminal",
                |f| f.terminal = None,
                "info",
                "Not Detected",
                true,
            ),
            (
                "disk",
                |f| f.disk_kb = Some(1_000_000),
                "warning",
                "977 MB Free",
                true,
            ),
            (
                "disk",
                |f| f.disk_kb = Some(100_000),
                "error",
                "98 MB Free",
                true,
            ),
            ("disk", |f| f.disk_kb = None, "info", "Unknown", false),
            ("network", |f| f.online = false, "warning", "Offline", true),
            (
                "waydroid_image",
                |f| f.initialized = false,
                "warning",
                "Not Initialized",
                true,
            ),
        ];
        for (key, edit, status, value, fix) in cases {
            let mut facts = healthy();
            edit(&mut facts);
            let got = check(&facts, key);
            assert_eq!(
                (got.0.as_str(), got.2),
                (*status, *fix),
                "{key} with {value}: {got:?}"
            );
            assert_eq!(got.1, *value, "{key}");
        }
    }

    #[test]
    fn privilege_elevation_covers_agent_tty_and_missing_tools() {
        let with = |edit: fn(&mut HostFacts)| {
            let mut facts = healthy();
            edit(&mut facts);
            check(&facts, "elevation")
        };
        // pkexec without an agent: sudo in a terminal can still ask.
        assert_eq!(with(|f| f.polkit_agent = false).0, "error");
        assert_eq!(
            with(|f| {
                f.polkit_agent = false;
                f.tty = true
            })
            .0,
            "warning"
        );
        // sudo only: needs the terminal ArchDroid was started from.
        let no_tty = with(|f| f.pkexec = false);
        assert_eq!(
            (no_tty.0.as_str(), no_tty.1.as_str()),
            ("error", "sudo Needs a TTY")
        );
        let tty = with(|f| {
            f.pkexec = false;
            f.tty = true
        });
        assert_eq!((tty.0.as_str(), tty.1.as_str()), ("ok", "sudo (terminal)"));
        assert_eq!(with(|_| {}).1, "Polkit (pkexec)");
        let none = with(|f| {
            f.pkexec = false;
            f.sudo = false
        });
        assert_eq!(
            (none.0.as_str(), none.1.as_str(), none.2),
            ("error", "Missing", true)
        );
    }

    #[test]
    fn without_a_package_manager_no_install_command_is_invented() {
        let mut facts = healthy();
        facts.pm = None;
        facts.git = false;
        assert_eq!(
            check(&facts, "git"),
            ("warning".into(), "Not Installed".into(), false)
        );
    }

    #[test]
    fn unavailable_waydroid_explains_where_to_get_it() {
        use crate::platform::PackageManager::*;
        assert!(super::waydroid_unavailable_detail(Some(Apt)).contains("sudo apt update"));
        assert!(super::waydroid_unavailable_detail(Some(Zypper)).contains("community OBS"));
        assert!(super::waydroid_unavailable_detail(Some(Dnf)).contains("EPEL"));
        assert!(super::waydroid_unavailable_detail(Some(Pacman)).contains("Re-check"));
    }

    #[test]
    fn missing_waydroid_on_apt_points_to_the_upstream_repository() {
        let apt = super::waydroid_missing_detail(Some(crate::platform::PackageManager::Apt));
        assert!(apt.contains("https://docs.waydro.id/usage/install-on-desktops"));
        let pacman = super::waydroid_missing_detail(Some(crate::platform::PackageManager::Pacman));
        assert!(!pacman.contains("repository"));
    }

    use super::*;

    #[test]
    fn check_host_diagnostics_returns_core_items() {
        let report = collect_diagnostics();
        assert!(report.items.len() >= 16);
        let keys: Vec<_> = report.items.iter().map(|i| i.key.as_str()).collect();
        assert!(keys.contains(&"session"));
        assert!(keys.contains(&"kvm"));
        assert!(keys.contains(&"binder"));
        assert!(keys.contains(&"gpu"));
        assert!(keys.contains(&"service"));
        assert!(keys.contains(&"waydroid"));
        assert!(keys.contains(&"adb"));
        assert!(keys.contains(&"elevation"));
        assert!(keys.contains(&"git"));
        assert!(keys.contains(&"python"));
        assert!(keys.contains(&"squashfs"));
        assert!(keys.contains(&"lzip"));
        assert!(keys.contains(&"xdg_open"));
        assert!(keys.contains(&"terminal"));
        assert!(keys.contains(&"disk"));
        assert!(keys.contains(&"network"));
        assert!(keys.contains(&"waydroid_image"));
    }
}
