//! Multi-distribution platform detection and package management.
//!
//! P0-7.1 explicitly defines supported Linux distributions (Arch, Debian/Ubuntu,
//! Fedora, openSUSE) and their package manager mappings. Installers detect the
//! host package manager and map package names appropriately. If dependencies are
//! missing on an unsupported distribution, installers fail fast before requesting root.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Pacman,
    Apt,
    Dnf,
    Zypper,
}

impl PackageManager {
    /// Detect the primary system package manager by checking trusted system paths.
    pub fn detect() -> Option<Self> {
        Self::detect_in(&["/usr/bin", "/bin", "/usr/sbin", "/sbin"])
    }

    /// Detect package manager looking inside given directories.
    pub fn detect_in(dirs: &[&str]) -> Option<Self> {
        for dir in dirs {
            let path = Path::new(dir);
            if path.join("pacman").is_file() {
                return Some(PackageManager::Pacman);
            }
            if path.join("apt-get").is_file() {
                return Some(PackageManager::Apt);
            }
            if path.join("dnf").is_file() {
                return Some(PackageManager::Dnf);
            }
            if path.join("zypper").is_file() {
                return Some(PackageManager::Zypper);
            }
        }
        None
    }

    pub fn name(&self) -> &'static str {
        match self {
            PackageManager::Pacman => "pacman",
            PackageManager::Apt => "apt-get",
            PackageManager::Dnf => "dnf",
            PackageManager::Zypper => "zypper",
        }
    }

    /// Map generic requirement identifier to the package name used by this distro.
    ///
    /// Note: openSUSE names the package providing `unsquashfs` `squashfs`, whereas
    /// Arch, Debian/Ubuntu, and Fedora name it `squashfs-tools`.
    pub fn package_name<'a>(&self, generic: &'a str) -> &'a str {
        match (self, generic) {
            (PackageManager::Zypper, "squashfs") => "squashfs",
            (_, "squashfs") => "squashfs-tools",
            (_, "lzip") => "lzip",
            (_, "waydroid") => "waydroid",
            (_, "git") => "git",
            (_, other) => other,
        }
    }

    /// Build the shell snippet to install missing packages with non-interactive flags.
    pub fn install_command(&self, packages: &[&str]) -> String {
        let mapped: Vec<&str> = packages.iter().map(|p| self.package_name(p)).collect();
        let pkgs = mapped.join(" ");
        match self {
            PackageManager::Pacman => format!("pacman -S --needed --noconfirm {pkgs}"),
            PackageManager::Apt => {
                format!("DEBIAN_FRONTEND=noninteractive apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends {pkgs}")
            }
            PackageManager::Dnf => format!("dnf install -y {pkgs}"),
            PackageManager::Zypper => {
                format!("zypper --non-interactive install --no-recommends {pkgs}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn package_name_mapping_respects_distro_differences() {
        assert_eq!(PackageManager::Pacman.package_name("squashfs"), "squashfs-tools");
        assert_eq!(PackageManager::Apt.package_name("squashfs"), "squashfs-tools");
        assert_eq!(PackageManager::Dnf.package_name("squashfs"), "squashfs-tools");
        assert_eq!(PackageManager::Zypper.package_name("squashfs"), "squashfs");

        for pm in [PackageManager::Pacman, PackageManager::Apt, PackageManager::Dnf, PackageManager::Zypper] {
            assert_eq!(pm.package_name("lzip"), "lzip");
            assert_eq!(pm.package_name("waydroid"), "waydroid");
            assert_eq!(pm.package_name("git"), "git");
        }
    }

    #[test]
    fn install_command_generates_valid_distro_syntax() {
        let pkgs = &["squashfs", "lzip"];
        assert_eq!(
            PackageManager::Pacman.install_command(pkgs),
            "pacman -S --needed --noconfirm squashfs-tools lzip"
        );
        assert_eq!(
            PackageManager::Apt.install_command(pkgs),
            "DEBIAN_FRONTEND=noninteractive apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends squashfs-tools lzip"
        );
        assert_eq!(
            PackageManager::Dnf.install_command(pkgs),
            "dnf install -y squashfs-tools lzip"
        );
        assert_eq!(
            PackageManager::Zypper.install_command(pkgs),
            "zypper --non-interactive install --no-recommends squashfs lzip"
        );
    }

    #[test]
    fn detect_in_identifies_distro_binaries() {
        let temp = std::env::temp_dir().join(format!("archdroid-pm-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();

        let temp_str = temp.to_str().unwrap();

        // Initially empty
        assert_eq!(PackageManager::detect_in(&[temp_str]), None);

        // Pacman
        let pacman_bin = temp.join("pacman");
        fs::write(&pacman_bin, "#!/bin/sh\n").unwrap();
        assert_eq!(PackageManager::detect_in(&[temp_str]), Some(PackageManager::Pacman));
        fs::remove_file(&pacman_bin).unwrap();

        // Apt
        let apt_bin = temp.join("apt-get");
        fs::write(&apt_bin, "#!/bin/sh\n").unwrap();
        assert_eq!(PackageManager::detect_in(&[temp_str]), Some(PackageManager::Apt));
        fs::remove_file(&apt_bin).unwrap();

        // Dnf
        let dnf_bin = temp.join("dnf");
        fs::write(&dnf_bin, "#!/bin/sh\n").unwrap();
        assert_eq!(PackageManager::detect_in(&[temp_str]), Some(PackageManager::Dnf));
        fs::remove_file(&dnf_bin).unwrap();

        // Zypper
        let zypper_bin = temp.join("zypper");
        fs::write(&zypper_bin, "#!/bin/sh\n").unwrap();
        assert_eq!(PackageManager::detect_in(&[temp_str]), Some(PackageManager::Zypper));

        let _ = fs::remove_dir_all(&temp);
    }
}
