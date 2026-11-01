//! Host diagnostics report behind the Settings > Diagnostics panel.

use std::path::Path;

use crate::*;

pub(crate) fn collect_diagnostics() -> DiagnosticsReport {
    let pm = platform::PackageManager::detect();
    let mut items = Vec::new();

    // 1. Session Type (Wayland vs X11)
    let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|s| s.to_lowercase() == "wayland")
            .unwrap_or(false);

    if is_wayland {
        items.push(DiagnosticItem {
            key: "session".to_string(),
            title: "Display Session".to_string(),
            status: "ok".to_string(),
            value: "Wayland Native".to_string(),
            detail: "Wayland compositor detected. Waydroid runs with native direct rendering."
                .to_string(),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "session".to_string(),
            title: "Display Session".to_string(),
            status: "warning".to_string(),
            value: "X11 Display".to_string(),
            detail: "Waydroid requires Wayland. Under X11, a nested compositor (Weston/cage) may be required.".to_string(),
            remediation: Some("Run inside a Wayland compositor, or use cage: cage -- waydroid show-full-ui".to_string()),
        });
    }

    // 2. KVM Virtualization Acceleration
    let kvm_path = Path::new("/dev/kvm");
    if kvm_path.exists() {
        let is_writable = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(kvm_path)
            .is_ok();

        if is_writable {
            items.push(DiagnosticItem {
                key: "kvm".to_string(),
                title: "KVM Acceleration".to_string(),
                status: "ok".to_string(),
                value: "Hardware KVM Ready".to_string(),
                detail: "/dev/kvm is available with read/write access.".to_string(),
                remediation: None,
            });
        } else {
            items.push(DiagnosticItem {
                key: "kvm".to_string(),
                title: "KVM Acceleration".to_string(),
                status: "warning".to_string(),
                value: "Permission Denied".to_string(),
                detail: "KVM device exists but user lacks write permission.".to_string(),
                remediation: Some("sudo usermod -aG kvm $USER".to_string()),
            });
        }
    } else {
        items.push(DiagnosticItem {
            key: "kvm".to_string(),
            title: "KVM Acceleration".to_string(),
            status: "warning".to_string(),
            value: "Not Detected".to_string(),
            detail:
                "/dev/kvm not found. Ensure Intel VT-x or AMD-V is enabled in motherboard BIOS."
                    .to_string(),
            remediation: Some(
                "Enable VT-x/AMD-V in BIOS and load kernel module: sudo modprobe kvm".to_string(),
            ),
        });
    }

    // 3. Android Binder IPC
    let has_binder = Path::new("/dev/binderfs").exists()
        || Path::new("/dev/binder").exists()
        || Path::new("/dev/binderfs/binder").exists();

    if has_binder {
        items.push(DiagnosticItem {
            key: "binder".to_string(),
            title: "Android Binder IPC".to_string(),
            status: "ok".to_string(),
            value: "Binder Nodes Active".to_string(),
            detail: "Android kernel inter-process communication drivers are active.".to_string(),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "binder".to_string(),
            title: "Android Binder IPC".to_string(),
            status: "ok".to_string(),
            value: "Managed by Daemon".to_string(),
            detail: "Binderfs will be mounted automatically when container daemon starts."
                .to_string(),
            remediation: None,
        });
    }

    // 4. GPU 3D Passthrough
    let has_gpu = Path::new("/dev/dri/renderD128").exists() || Path::new("/dev/dri").exists();
    if has_gpu {
        items.push(DiagnosticItem {
            key: "gpu".to_string(),
            title: "GPU Acceleration".to_string(),
            status: "ok".to_string(),
            value: "Direct Rendering (DRI)".to_string(),
            detail: "Hardware Direct Rendering Manager node is available for 3D graphics."
                .to_string(),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "gpu".to_string(),
            title: "GPU Acceleration".to_string(),
            status: "warning".to_string(),
            value: "Software Rendering".to_string(),
            detail: "Hardware DRI node not found. Android graphics will fall back to software rendering.".to_string(),
            remediation: Some("Install graphics drivers supporting DRI (Direct Rendering Infrastructure).".to_string()),
        });
    }

    // 5. Waydroid Container Service
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

    if service_active {
        items.push(DiagnosticItem {
            key: "service".to_string(),
            title: "Container Daemon".to_string(),
            status: "ok".to_string(),
            value: "Service Active".to_string(),
            detail: "waydroid-container.service is actively running in background.".to_string(),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "service".to_string(),
            title: "Container Daemon".to_string(),
            status: "info".to_string(),
            value: "Service Inactive".to_string(),
            detail:
                "Container daemon is inactive. It will start automatically when session starts."
                    .to_string(),
            remediation: None,
        });
    }

    // 6. Waydroid CLI binary
    let has_waydroid =
        resolve_program("waydroid").is_some() || trusted_program("waydroid").is_some();
    if has_waydroid {
        items.push(DiagnosticItem {
            key: "waydroid".to_string(),
            title: "Waydroid Package".to_string(),
            status: "ok".to_string(),
            value: "Installed".to_string(),
            detail: "Waydroid command-line tool is available in system PATH.".to_string(),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "waydroid".to_string(),
            title: "Waydroid Package".to_string(),
            status: "error".to_string(),
            value: "Not Installed".to_string(),
            detail: waydroid_missing_detail(pm),
            remediation: pm.map(|p| p.user_install_command(&["waydroid"])),
        });
    }

    // 7. ADB / android-tools
    let has_adb = resolve_program("adb").is_some() || trusted_program("adb").is_some();
    if has_adb {
        items.push(DiagnosticItem {
            key: "adb".to_string(),
            title: "Android Debug Bridge".to_string(),
            status: "ok".to_string(),
            value: "Installed".to_string(),
            detail: "adb is available for unprivileged container queries and inspection."
                .to_string(),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "adb".to_string(),
            title: "Android Debug Bridge".to_string(),
            status: "warning".to_string(),
            value: "Not Installed".to_string(),
            detail: "adb is missing. App list, install/uninstall, and Android boot detection fall back to slower or unavailable paths.".to_string(),
            remediation: pm.map(|p| p.user_install_command(&["adb"])),
        });
    }

    // 8. Root Elevation Tool (polkit / sudo)
    let has_pkexec = has_program("pkexec");
    let has_sudo = has_program("sudo");
    if has_pkexec && !polkit_agent_running() {
        items.push(DiagnosticItem {
            key: "elevation".to_string(),
            title: "Privilege Elevation".to_string(),
            status: if has_sudo && std::io::stdin().is_terminal() {
                "warning"
            } else {
                "error"
            }
            .to_string(),
            value: "No Polkit Agent".to_string(),
            detail: "pkexec is installed but no polkit authentication agent is running, so the password prompt cannot appear. Start an agent in your session, or launch ArchDroid from a terminal so sudo can ask instead.".to_string(),
            remediation: Some(
                "Start one, e.g. hyprpolkitagent, polkit-gnome-authentication-agent-1, or lxqt-policykit-agent"
                    .to_string(),
            ),
        });
    } else if !has_pkexec && has_sudo && !std::io::stdin().is_terminal() {
        items.push(DiagnosticItem {
            key: "elevation".to_string(),
            title: "Privilege Elevation".to_string(),
            status: "error".to_string(),
            value: "sudo Needs a TTY".to_string(),
            detail: "Only sudo is available and ArchDroid has no terminal to ask for the password. Administrative operations will fail.".to_string(),
            remediation: Some(format!(
                "{}, or start ArchDroid from a terminal",
                pm.map(|p| p.user_install_command(&["polkit"]))
                    .unwrap_or_else(|| "Install polkit and a polkit agent".to_string())
            )),
        });
    } else if let Some(tool) = has_elevation_tool() {
        items.push(DiagnosticItem {
            key: "elevation".to_string(),
            title: "Privilege Elevation".to_string(),
            status: "ok".to_string(),
            value: match tool {
                "pkexec" => "Polkit (pkexec)".to_string(),
                _ => "sudo (terminal)".to_string(),
            },
            detail: format!(
                "Authentication agent '{tool}' is available for administrative operations."
            ),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "elevation".to_string(),
            title: "Privilege Elevation".to_string(),
            status: "error".to_string(),
            value: "Missing".to_string(),
            detail: "Neither pkexec nor sudo was found. Administrative operations will fail."
                .to_string(),
            remediation: pm.map(|p| p.user_install_command(&["polkit"])),
        });
    }

    // 9. Git
    let has_git = resolve_program("git").is_some() || trusted_program("git").is_some();
    if has_git {
        items.push(DiagnosticItem {
            key: "git".to_string(),
            title: "Git Version Control".to_string(),
            status: "ok".to_string(),
            value: "Installed".to_string(),
            detail: "git is available to download pinned third-party helper checkouts.".to_string(),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "git".to_string(),
            title: "Git Version Control".to_string(),
            status: "warning".to_string(),
            value: "Not Installed".to_string(),
            detail: "git is required by GApps, ARM Translation, and Phantom installers."
                .to_string(),
            remediation: pm.map(|p| p.user_install_command(&["git"])),
        });
    }

    // 10. Python 3 & venv
    let (has_py, has_venv) = check_python_venv_available();
    if has_py && has_venv {
        items.push(DiagnosticItem {
            key: "python".to_string(),
            title: "Python 3 & venv".to_string(),
            status: "ok".to_string(),
            value: "Ready".to_string(),
            detail: "Python 3 and venv module are ready for isolated script execution.".to_string(),
            remediation: None,
        });
    } else if has_py {
        items.push(DiagnosticItem {
            key: "python".to_string(),
            title: "Python 3 & venv".to_string(),
            status: "error".to_string(),
            value: "venv Missing".to_string(),
            detail: "Python 3 is present but python3-venv is missing. Debian/Ubuntu systems require python3-venv.".to_string(),
            remediation: pm.map(|p| p.user_install_command(&["python3-venv"])),
        });
    } else {
        items.push(DiagnosticItem {
            key: "python".to_string(),
            title: "Python 3 & venv".to_string(),
            status: "error".to_string(),
            value: "Not Installed".to_string(),
            detail: "Python 3 is required by GApps and ARM Translation installers.".to_string(),
            remediation: pm.map(|p| p.user_install_command(&["python3", "python3-venv"])),
        });
    }

    // 11. squashfs extraction tool
    let has_squash =
        resolve_program("unsquashfs").is_some() || trusted_program("unsquashfs").is_some();
    if has_squash {
        items.push(DiagnosticItem {
            key: "squashfs".to_string(),
            title: "SquashFS Tools".to_string(),
            status: "ok".to_string(),
            value: "unsquashfs Ready".to_string(),
            detail: "unsquashfs is available to unpack Android filesystem images.".to_string(),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "squashfs".to_string(),
            title: "SquashFS Tools".to_string(),
            status: "warning".to_string(),
            value: "Missing".to_string(),
            detail:
                "unsquashfs is required to extract Android images for GApps and ARM Translation."
                    .to_string(),
            remediation: pm.map(|p| p.user_install_command(&["squashfs"])),
        });
    }

    // 12. lzip
    let has_lzip = resolve_program("lzip").is_some() || trusted_program("lzip").is_some();
    if has_lzip {
        items.push(DiagnosticItem {
            key: "lzip".to_string(),
            title: "Lzip Decompressor".to_string(),
            status: "ok".to_string(),
            value: "lzip Ready".to_string(),
            detail: "lzip is available to extract MindTheGapps and libhoudini archives."
                .to_string(),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "lzip".to_string(),
            title: "Lzip Decompressor".to_string(),
            status: "warning".to_string(),
            value: "Missing".to_string(),
            detail: "lzip is required to decompress archives for GApps and ARM Translation."
                .to_string(),
            remediation: pm.map(|p| p.user_install_command(&["lzip"])),
        });
    }

    // 13. xdg-open
    let has_xdg = resolve_program("xdg-open").is_some() || trusted_program("xdg-open").is_some();
    if has_xdg {
        items.push(DiagnosticItem {
            key: "xdg_open".to_string(),
            title: "File Manager Launcher".to_string(),
            status: "ok".to_string(),
            value: "xdg-open Ready".to_string(),
            detail: "xdg-open is available to open shared storage in your default file manager."
                .to_string(),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "xdg_open".to_string(),
            title: "File Manager Launcher".to_string(),
            status: "info".to_string(),
            value: "Missing".to_string(),
            detail: "xdg-open is needed to open the Android shared media folder in a file manager."
                .to_string(),
            remediation: pm.map(|p| p.user_install_command(&["xdg-utils"])),
        });
    }

    // 14. Terminal Emulator for Phantom
    let terminal = find_terminal_emulator();
    if let Some(ref term) = terminal {
        items.push(DiagnosticItem {
            key: "terminal".to_string(),
            title: "Terminal Emulator".to_string(),
            status: "ok".to_string(),
            value: term.clone(),
            detail: format!(
                "Terminal emulator '{term}' detected to run Phantom Keymapper interactive shell."
            ),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "terminal".to_string(),
            title: "Terminal Emulator".to_string(),
            status: "info".to_string(),
            value: "Not Detected".to_string(),
            detail: "A desktop terminal emulator (alacritty, foot, kitty, konsole, gnome-terminal) is needed to launch Phantom Keymapper.".to_string(),
            remediation: pm.map(|p| p.user_install_command(&["alacritty"])),
        });
    }

    // 15. Disk Space
    let disk_kb = check_available_disk_space_kb(Path::new("/var/lib/waydroid"));
    if let Some(kb) = disk_kb {
        let (status, detail, remediation) = if kb >= 1_500_000 {
            (
                "ok",
                "Sufficient disk space available for system images and container operations.",
                None,
            )
        } else if kb >= 500_000 {
            (
                "warning",
                "Low disk space (< 1.5 GB). Initializing Waydroid requires ~1.2 GB of free space.",
                Some("Free up at least 1.5 GB on host root or /var filesystem.".to_string()),
            )
        } else {
            (
                "error",
                "Critically low disk space (< 500 MB). Waydroid container operations may fail.",
                Some("Free up disk space immediately on host root or /var filesystem.".to_string()),
            )
        };
        items.push(DiagnosticItem {
            key: "disk".to_string(),
            title: "Available Storage".to_string(),
            status: status.to_string(),
            value: format_disk_space(kb),
            detail: detail.to_string(),
            remediation,
        });
    } else {
        items.push(DiagnosticItem {
            key: "disk".to_string(),
            title: "Available Storage".to_string(),
            status: "info".to_string(),
            value: "Unknown".to_string(),
            detail: "Unable to query available disk space via df.".to_string(),
            remediation: None,
        });
    }

    // 16. Network Connectivity
    let online = check_network_connectivity();
    if online {
        items.push(DiagnosticItem {
            key: "network".to_string(),
            title: "Network Connection".to_string(),
            status: "ok".to_string(),
            value: "Connected".to_string(),
            detail: "Internet connectivity verified. Online downloads (system images, GApps, ARM translation) ready.".to_string(),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "network".to_string(),
            title: "Network Connection".to_string(),
            status: "warning".to_string(),
            value: "Offline".to_string(),
            detail: "No internet connection detected. Offline host cannot download container images or third-party packages.".to_string(),
            remediation: Some("Connect your Linux host to the internet to download required system images and packages.".to_string()),
        });
    }

    // 17. Waydroid Android image initialization
    if waydroid::is_initialized() {
        items.push(DiagnosticItem {
            key: "waydroid_image".to_string(),
            title: "Android System Image".to_string(),
            status: "ok".to_string(),
            value: "Initialized".to_string(),
            detail: "The Waydroid Android image is initialized and ready for sessions and add-ons."
                .to_string(),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "waydroid_image".to_string(),
            title: "Android System Image".to_string(),
            status: "warning".to_string(),
            value: "Not Initialized".to_string(),
            detail: "The Android image has not been initialized. GApps and ARM Translation need an initialized image."
                .to_string(),
            remediation: Some(
                "Run the Setup Wizard, or run: sudo waydroid init".to_string(),
            ),
        });
    }

    DiagnosticsReport {
        items,
        package_manager: pm.map(|p| p.name().to_string()),
    }
}

/// Debian and Ubuntu releases before Waydroid entered their archives (e.g.
/// Ubuntu 24.04) have no `waydroid` package, so the apt command alone fails
/// with "Unable to locate package"; upstream documents its own repository.
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
