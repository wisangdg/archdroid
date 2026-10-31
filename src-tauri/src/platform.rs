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
            (PackageManager::Apt, "adb") => "adb",
            (_, "adb") => "android-tools",
            (PackageManager::Pacman, "python3") => "python",
            (_, "python3") => "python3",
            (PackageManager::Pacman, "python3-venv") => "python",
            (PackageManager::Apt, "python3-venv") => "python3-venv",
            (_, "python3-venv") => "python3",
            (_, "xdg-utils") => "xdg-utils",
            (PackageManager::Apt, "polkit") => "policykit-1",
            (_, "polkit") => "polkit",
            (PackageManager::Pacman, "cargo") => "rust",
            (_, "cargo") => "cargo",
            (PackageManager::Pacman, "jdk") => "jdk-openjdk",
            (PackageManager::Apt, "jdk") => "default-jdk-headless",
            (_, "jdk") => "java-17-openjdk-devel",
            (_, "coreutils") => "coreutils",
            (_, "sed") => "sed",
            (_, other) => other,
        }
    }

    /// Build the shell snippet to install missing packages with non-interactive
    /// flags. Production code previews and runs the already-mapped list through
    /// `install_mapped_command`, so this wrapper only serves the mapping tests.
    #[cfg(test)]
    pub fn install_command(&self, packages: &[&str]) -> String {
        let mapped = self.mapped_packages(packages);
        self.install_mapped_command(&mapped)
    }

    /// Build the shell snippet from names that were already mapped for this
    /// package manager. Preview and execution both use this so a distro package
    /// name is never passed through the generic mapping twice.
    pub fn install_mapped_command(&self, packages: &[&str]) -> String {
        let pkgs = packages.join(" ");
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

    /// Build human-readable command for user remediation in terminal (prefixed with sudo).
    pub fn user_install_command(&self, packages: &[&str]) -> String {
        let mapped = self.mapped_packages(packages);
        self.user_install_mapped_command(&mapped)
    }

    /// Human-readable remediation command for names already mapped for this
    /// package manager.
    pub fn user_install_mapped_command(&self, packages: &[&str]) -> String {
        let pkgs = packages.join(" ");
        match self {
            PackageManager::Pacman => format!("sudo pacman -S --needed {pkgs}"),
            PackageManager::Apt => {
                format!("sudo apt-get install -y --no-install-recommends {pkgs}")
            }
            PackageManager::Dnf => format!("sudo dnf install -y {pkgs}"),
            PackageManager::Zypper => {
                format!("sudo zypper install -y -n {pkgs}")
            }
        }
    }

    /// Map generic requirements to distro package names, dropping duplicates
    /// such as Arch's `python3` and `python3-venv`, which are both `python`.
    pub fn mapped_packages<'a>(&self, packages: &[&'a str]) -> Vec<&'a str> {
        let mut mapped: Vec<&str> = Vec::new();
        for package in packages {
            let name = self.package_name(package);
            if !mapped.contains(&name) {
                mapped.push(name);
            }
        }
        mapped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn package_name_mapping_respects_distro_differences() {
        assert_eq!(
            PackageManager::Pacman.package_name("squashfs"),
            "squashfs-tools"
        );
        assert_eq!(
            PackageManager::Apt.package_name("squashfs"),
            "squashfs-tools"
        );
        assert_eq!(
            PackageManager::Dnf.package_name("squashfs"),
            "squashfs-tools"
        );
        assert_eq!(PackageManager::Zypper.package_name("squashfs"), "squashfs");

        for pm in [
            PackageManager::Pacman,
            PackageManager::Apt,
            PackageManager::Dnf,
            PackageManager::Zypper,
        ] {
            assert_eq!(pm.package_name("lzip"), "lzip");
            assert_eq!(pm.package_name("waydroid"), "waydroid");
            assert_eq!(pm.package_name("git"), "git");
        }

        assert_eq!(PackageManager::Apt.package_name("adb"), "adb");
        assert_eq!(PackageManager::Pacman.package_name("adb"), "android-tools");
        assert_eq!(PackageManager::Dnf.package_name("adb"), "android-tools");
        assert_eq!(PackageManager::Zypper.package_name("adb"), "android-tools");

        assert_eq!(PackageManager::Pacman.package_name("python3"), "python");
        assert_eq!(PackageManager::Apt.package_name("python3"), "python3");
        assert_eq!(
            PackageManager::Pacman.package_name("python3-venv"),
            "python"
        );
        assert_eq!(
            PackageManager::Apt.package_name("python3-venv"),
            "python3-venv"
        );
        assert_eq!(PackageManager::Dnf.package_name("python3-venv"), "python3");

        assert_eq!(PackageManager::Pacman.package_name("cargo"), "rust");
        assert_eq!(PackageManager::Apt.package_name("cargo"), "cargo");
        assert_eq!(PackageManager::Dnf.package_name("cargo"), "cargo");
        assert_eq!(PackageManager::Zypper.package_name("cargo"), "cargo");

        assert_eq!(PackageManager::Pacman.package_name("jdk"), "jdk-openjdk");
        assert_eq!(
            PackageManager::Apt.package_name("jdk"),
            "default-jdk-headless"
        );
        assert_eq!(
            PackageManager::Dnf.package_name("jdk"),
            "java-17-openjdk-devel"
        );
        assert_eq!(
            PackageManager::Zypper.package_name("jdk"),
            "java-17-openjdk-devel"
        );

        assert_eq!(
            PackageManager::Pacman.package_name("coreutils"),
            "coreutils"
        );
        assert_eq!(PackageManager::Apt.package_name("sed"), "sed");
    }

    #[test]
    fn user_install_command_formats_sudo_terminal_command() {
        let pkgs = &["waydroid"];
        assert_eq!(
            PackageManager::Pacman.user_install_command(pkgs),
            "sudo pacman -S --needed waydroid"
        );
        assert_eq!(
            PackageManager::Apt.user_install_command(pkgs),
            "sudo apt-get install -y --no-install-recommends waydroid"
        );
        assert_eq!(
            PackageManager::Dnf.user_install_command(pkgs),
            "sudo dnf install -y waydroid"
        );
        assert_eq!(
            PackageManager::Zypper.user_install_command(pkgs),
            "sudo zypper install -y -n waydroid"
        );
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
    fn package_mapping_deduplicates_shared_names() {
        let pkgs = &["python3", "python3-venv"];
        assert_eq!(
            PackageManager::Pacman.install_command(pkgs),
            "pacman -S --needed --noconfirm python"
        );
        assert_eq!(
            PackageManager::Apt.install_command(pkgs),
            "DEBIAN_FRONTEND=noninteractive apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends python3 python3-venv"
        );
        assert_eq!(
            PackageManager::Dnf.install_command(pkgs),
            "dnf install -y python3"
        );
        assert_eq!(
            PackageManager::Zypper.install_command(pkgs),
            "zypper --non-interactive install --no-recommends python3"
        );
        assert_eq!(
            PackageManager::Pacman.user_install_command(pkgs),
            "sudo pacman -S --needed python"
        );
    }

    #[test]
    fn mapped_commands_do_not_remap_distro_package_names() {
        let pacman = PackageManager::Pacman;
        assert_eq!(
            pacman.mapped_packages(&["python3", "python3-venv", "squashfs"]),
            vec!["python", "squashfs-tools"]
        );
        assert_eq!(
            pacman.install_mapped_command(&["python", "squashfs-tools"]),
            "pacman -S --needed --noconfirm python squashfs-tools"
        );
        assert_eq!(
            pacman.user_install_mapped_command(&["python", "squashfs-tools"]),
            "sudo pacman -S --needed python squashfs-tools"
        );

        let apt = PackageManager::Apt;
        assert_eq!(
            apt.mapped_packages(&["python3", "python3-venv", "squashfs"]),
            vec!["python3", "python3-venv", "squashfs-tools"]
        );
        assert_eq!(
            apt.install_mapped_command(&["python3", "python3-venv", "squashfs-tools"]),
            "DEBIAN_FRONTEND=noninteractive apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends python3 python3-venv squashfs-tools"
        );
    }

    #[test]
    fn install_commands_never_upgrade_the_whole_system() {
        for pm in [
            PackageManager::Pacman,
            PackageManager::Apt,
            PackageManager::Dnf,
            PackageManager::Zypper,
        ] {
            let command = pm.install_command(&["git"]);
            assert!(!command.contains("Syu"), "{command}");
            assert!(!command.contains("dist-upgrade"), "{command}");
            assert!(!command.contains("full-upgrade"), "{command}");
            assert!(!command.contains("dup"), "{command}");
            assert!(!command.contains("upgrade"), "{command}");
            assert!(!command.contains("update\n"), "{command}");
        }
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
        assert_eq!(
            PackageManager::detect_in(&[temp_str]),
            Some(PackageManager::Pacman)
        );
        fs::remove_file(&pacman_bin).unwrap();

        // Apt
        let apt_bin = temp.join("apt-get");
        fs::write(&apt_bin, "#!/bin/sh\n").unwrap();
        assert_eq!(
            PackageManager::detect_in(&[temp_str]),
            Some(PackageManager::Apt)
        );
        fs::remove_file(&apt_bin).unwrap();

        // Dnf
        let dnf_bin = temp.join("dnf");
        fs::write(&dnf_bin, "#!/bin/sh\n").unwrap();
        assert_eq!(
            PackageManager::detect_in(&[temp_str]),
            Some(PackageManager::Dnf)
        );
        fs::remove_file(&dnf_bin).unwrap();

        // Zypper
        let zypper_bin = temp.join("zypper");
        fs::write(&zypper_bin, "#!/bin/sh\n").unwrap();
        assert_eq!(
            PackageManager::detect_in(&[temp_str]),
            Some(PackageManager::Zypper)
        );

        let _ = fs::remove_dir_all(&temp);
    }
}
