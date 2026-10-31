//! Host dependency checks and the preflight gates that run before any
//! installer asks for root, plus the package-install plan shown for approval.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::AppError;
use crate::*;

pub(crate) fn check_network_connectivity() -> bool {
    let targets = [
        "1.1.1.1:53".parse::<std::net::SocketAddr>().unwrap(),
        "8.8.8.8:53".parse::<std::net::SocketAddr>().unwrap(),
    ];
    for target in &targets {
        if std::net::TcpStream::connect_timeout(target, std::time::Duration::from_secs(1)).is_ok() {
            return true;
        }
    }
    false
}

pub(crate) fn check_available_disk_space_kb(path: &Path) -> Option<u64> {
    let probe_path = if path.exists() {
        path
    } else if let Some(parent) = path.parent() {
        if parent.exists() {
            parent
        } else {
            Path::new("/")
        }
    } else {
        Path::new("/")
    };

    let output = Command::new("df")
        .args(["-Pk", probe_path.to_str().unwrap_or("/")])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            if let Ok(avail) = parts[3].parse::<u64>() {
                return Some(avail);
            }
        }
    }
    None
}

pub(crate) fn format_disk_space(kb: u64) -> String {
    if kb >= 1_048_576 {
        format!("{:.1} GB Free", kb as f64 / 1_048_576.0)
    } else {
        format!("{:.0} MB Free", kb as f64 / 1024.0)
    }
}

pub(crate) fn check_python_venv_available() -> (bool, bool) {
    let has_python3 = resolve_program("python3").is_some() || trusted_program("python3").is_some();
    if !has_python3 {
        return (false, false);
    }
    let has_venv = Command::new("python3")
        .args(["-c", "import venv, ensurepip"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    (true, has_venv)
}

/// Root scripts run with `SAFE_PATH`, so preflight must not accept a `python3`
/// from a user-writable directory that disappears after elevation.
pub(crate) fn check_trusted_python_venv() -> (bool, bool) {
    let Some(python) = trusted_program("python3") else {
        return (false, false);
    };
    let has_venv = Command::new(python)
        .args(["-c", "import venv, ensurepip"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    (true, has_venv)
}

pub(crate) fn find_terminal_emulator() -> Option<String> {
    if let Ok(term) = std::env::var("TERMINAL") {
        let trimmed = term.trim();
        if !trimmed.is_empty()
            && (resolve_program(trimmed).is_some() || trusted_program(trimmed).is_some())
        {
            return Some(trimmed.to_string());
        }
    }
    let candidates = [
        "konsole",
        "alacritty",
        "gnome-terminal",
        "foot",
        "kitty",
        "ghostty",
        "xfce4-terminal",
        "xterm",
    ];
    for candidate in candidates {
        if resolve_program(candidate).is_some() || trusted_program(candidate).is_some() {
            return Some(candidate.to_string());
        }
    }
    None
}

pub(crate) fn has_elevation_tool() -> Option<&'static str> {
    if resolve_program("pkexec").is_some() || trusted_program("pkexec").is_some() {
        Some("pkexec")
    } else if resolve_program("sudo").is_some() || trusted_program("sudo").is_some() {
        Some("sudo")
    } else {
        None
    }
}

/// True when a polkit authentication agent process is running. `pkexec` has
/// nowhere to show its password prompt without one.
pub(crate) fn polkit_agent_running() -> bool {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    entries.flatten().any(|entry| {
        entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.bytes().all(|b| b.is_ascii_digit()))
            && std::fs::read_to_string(entry.path().join("comm"))
                .map(|comm| is_polkit_agent_name(comm.trim()))
                .unwrap_or(false)
    })
}

pub(crate) fn is_polkit_agent_name(comm: &str) -> bool {
    let comm = comm.to_ascii_lowercase();
    // `polkitd` is the daemon, not an authentication agent.
    comm != "polkitd" && (comm.contains("polkit") || comm.contains("policykit"))
}

pub(crate) fn has_program(name: &str) -> bool {
    resolve_program(name).is_some() || trusted_program(name).is_some()
}

/// Android SDK root, preferring the environment variables the upstream Phantom
/// build script already understands.
pub(crate) fn android_sdk_root() -> Option<PathBuf> {
    for key in ["ANDROID_SDK_ROOT", "ANDROID_HOME"] {
        if let Some(value) = std::env::var_os(key) {
            let path = PathBuf::from(value);
            if path.is_dir() {
                return Some(path);
            }
        }
    }
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let path = home.join("Android/Sdk");
    path.is_dir().then_some(path)
}

/// `android.jar` from `ANDROID_JAR` or a platform under the Android SDK.
pub(crate) fn find_android_jar() -> Option<PathBuf> {
    if let Some(value) = std::env::var_os("ANDROID_JAR") {
        let path = PathBuf::from(value);
        if path.is_file() {
            return Some(path);
        }
    }
    let platforms = android_sdk_root()?.join("platforms");
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(platforms)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path().join("android.jar"))
        .filter(|path| path.is_file())
        .collect();
    candidates.sort();
    candidates.pop()
}

/// `d8` from `D8`, PATH, the SDK build-tools, or cmdline-tools.
pub(crate) fn find_d8() -> Option<PathBuf> {
    if let Some(value) = std::env::var_os("D8") {
        let path = PathBuf::from(value);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Some(path) = resolve_program("d8").or_else(|| trusted_program("d8")) {
        return Some(path);
    }
    let sdk = android_sdk_root()?;
    if let Ok(entries) = std::fs::read_dir(sdk.join("build-tools")) {
        let mut candidates: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path().join("d8"))
            .filter(|path| path.is_file())
            .collect();
        candidates.sort();
        if let Some(path) = candidates.pop() {
            return Some(path);
        }
    }
    let cmdline_d8 = sdk.join("cmdline-tools/latest/bin/d8");
    cmdline_d8.is_file().then_some(cmdline_d8)
}

/// Generic dependency names shared by the GApps and ARM Translation installers.
/// Root scripts run with `SAFE_PATH`, so only trusted system locations count.
pub(crate) fn gapps_missing_dependencies() -> Vec<&'static str> {
    let mut missing = Vec::new();
    if trusted_program("git").is_none() {
        missing.push("git");
    }
    let (has_python3, has_venv) = check_trusted_python_venv();
    if !has_python3 {
        missing.push("python3");
    } else if !has_venv {
        missing.push("python3-venv");
    }
    if trusted_program("unsquashfs").is_none() {
        missing.push("squashfs");
    }
    if trusted_program("lzip").is_none() {
        missing.push("lzip");
    }
    missing
}

/// Actions that may need host packages before their privileged work starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PackageInstallAction {
    Init,
    Gapps,
    Houdini,
}

impl PackageInstallAction {
    pub(crate) fn parse(action: &str) -> error::Result<Self> {
        match action {
            "init" => Ok(Self::Init),
            "gapps" => Ok(Self::Gapps),
            "houdini" => Ok(Self::Houdini),
            other => Err(AppError::invalid_input(format!(
                "Unknown package installation action \"{other}\". Expected init, gapps, or houdini."
            ))),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Init => "init",
            Self::Gapps => "gapps",
            Self::Houdini => "houdini",
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Init => "initializing Waydroid",
            Self::Gapps => "installing GApps",
            Self::Houdini => "installing ARM Translation",
        }
    }

    /// Generic dependency names that are missing right now. The package manager
    /// maps these to distro package names before anything is shown or run.
    pub(crate) fn missing_packages(self) -> Vec<&'static str> {
        match self {
            Self::Init => {
                if trusted_program("waydroid").is_none() {
                    vec!["waydroid"]
                } else {
                    Vec::new()
                }
            }
            Self::Gapps | Self::Houdini => gapps_missing_dependencies(),
        }
    }
}

pub(crate) fn package_install_preview(
    action: PackageInstallAction,
    missing: &[&'static str],
    pm: Option<platform::PackageManager>,
) -> PackageInstallPreview {
    match pm {
        Some(pm) => {
            let mapped = pm.mapped_packages(missing);
            PackageInstallPreview {
                action: action.as_str().to_string(),
                package_manager: Some(pm.name().to_string()),
                packages: mapped.iter().map(|package| package.to_string()).collect(),
                command: (!mapped.is_empty()).then(|| pm.install_mapped_command(&mapped)),
                requires_root: true,
            }
        }
        None => PackageInstallPreview {
            action: action.as_str().to_string(),
            package_manager: None,
            packages: missing.iter().map(|package| package.to_string()).collect(),
            command: None,
            requires_root: true,
        },
    }
}

/// Validate the package list the user saw and approved against what the
/// installer still needs. Only exact distro names from the reviewed plan are
/// accepted, so an IPC caller cannot smuggle an arbitrary package or shell
/// fragment into the root script.
pub(crate) fn approved_install_packages(
    action: PackageInstallAction,
    required: &[&'static str],
    pm: Option<platform::PackageManager>,
    approved: Option<&[String]>,
) -> error::Result<Vec<&'static str>> {
    let Some(pm) = pm else {
        // The existing preflight reports the missing package manager before
        // elevation; there is no install plan to approve in that case.
        return Ok(Vec::new());
    };

    let mapped = pm.mapped_packages(required);
    if mapped.is_empty() {
        return Ok(Vec::new());
    }

    let approved = approved.ok_or_else(|| {
        AppError::approval_required(format!(
            "Installing system packages for {} requires explicit approval. Review the package preview and confirm again.",
            action.description()
        ))
    })?;

    let mut accepted: Vec<&'static str> = Vec::new();
    for name in approved {
        let matched = mapped
            .iter()
            .copied()
            .find(|candidate| *candidate == name)
            .ok_or_else(|| {
                AppError::invalid_input(format!(
                    "Package \"{name}\" is not part of the reviewed installation plan for {}. Review the package preview again.",
                    action.description()
                ))
            })?;
        if !accepted.contains(&matched) {
            accepted.push(matched);
        }
    }

    let unapproved: Vec<&str> = mapped
        .iter()
        .copied()
        .filter(|name| !accepted.contains(name))
        .collect();
    if !unapproved.is_empty() {
        return Err(AppError::approval_required(format!(
            "The following required packages were not approved: {}. Review the package preview and confirm again.",
            unapproved.join(", ")
        )));
    }

    Ok(accepted)
}

/// Probing for missing tools spawns `python3` and friends, so it runs off the
/// main thread like every other host probe.
/// Both GApps and Houdini modify an existing Android image. Refuse before the
/// root prompt when the image is absent: the upstream script cannot repair that.
pub(crate) fn require_initialized_waydroid(action: &str) -> Result<(), AppError> {
    if trusted_program("waydroid").is_none() {
        return Err(AppError::not_found(
            "waydroid",
            &format!("Waydroid must be installed in a system path before {action}."),
        ));
    }
    if !waydroid::is_initialized() {
        return Err(AppError::not_initialized(format!(
            "Waydroid is not initialized. Run the setup wizard or `sudo waydroid init` before {action}."
        )));
    }
    Ok(())
}

pub(crate) fn installer_preflight(
    pm: Option<platform::PackageManager>,
    action: &str,
) -> Result<(), AppError> {
    if has_elevation_tool().is_none() {
        return Err(AppError::elevation_denied(
            "No root elevation tool (pkexec or sudo) found on host. Cannot perform administrative operations.",
        ));
    }
    if !check_network_connectivity() {
        return Err(AppError::command_failed(format!(
            "No internet connection detected. Internet access is required to {action}."
        )));
    }
    require_initialized_waydroid(action)?;
    if let Some(kb) = check_available_disk_space_kb(Path::new("/var/lib/waydroid")) {
        if kb < 500_000 {
            return Err(AppError::command_failed(format!(
                "Insufficient disk space: only {:.0} MB available. {} requires at least 500 MB.",
                kb as f64 / 1024.0,
                action
            )));
        }
    }
    let missing = gapps_missing_dependencies();
    if !missing.is_empty() && pm.is_none() {
        let names = missing.join(", ");
        return Err(AppError::not_found(
            &names,
            &format!(
                "Required dependencies ({names}) are missing, and no supported package manager (pacman, apt-get, dnf, zypper) was found on this Linux distribution. Please install them manually before {action}."
            ),
        ));
    }
    Ok(())
}

/// Phantom builds Rust binaries and an Android server jar as the desktop user.
/// None of these can be fixed after the build starts, so collect them first.
pub(crate) fn phantom_missing_dependencies() -> (Vec<&'static str>, Vec<&'static str>) {
    let mut packages = Vec::new();
    let mut sdk = Vec::new();
    if !has_program("git") {
        packages.push("git");
    }
    if !has_program("cargo") {
        packages.push("cargo");
    }
    if !has_program("javac") || !has_program("jar") {
        packages.push("jdk");
    }
    if !has_program("sed") {
        packages.push("sed");
    }
    if !has_program("install") {
        packages.push("coreutils");
    }
    if find_d8().is_none() {
        sdk.push("d8");
    }
    if find_android_jar().is_none() {
        sdk.push("android.jar");
    }
    (packages, sdk)
}

pub(crate) fn phantom_dependency_error(
    packages: &[&'static str],
    sdk: &[&'static str],
    pm: Option<platform::PackageManager>,
) -> AppError {
    let mut names = packages.to_vec();
    names.extend(sdk.iter().copied());

    let mut hints = Vec::new();
    if !packages.is_empty() {
        match pm {
            Some(pm) => hints.push(format!(
                "Install missing packages with: {}",
                pm.user_install_command(packages)
            )),
            None => hints.push(format!(
                "Install missing packages ({}) with your distribution's package manager.",
                packages.join(", ")
            )),
        }
    }
    if sdk.contains(&"d8") {
        hints.push(
            "Install Android SDK command-line tools so `d8` is on PATH, or set D8=/path/to/d8."
                .to_string(),
        );
    }
    if sdk.contains(&"android.jar") {
        hints.push(
            "Install an Android platform with sdkmanager (for example `sdkmanager \"platforms;android-34\"`), or set ANDROID_JAR=/path/to/android.jar."
                .to_string(),
        );
    }

    AppError::not_found(
        &names.join(", "),
        &format!(
            "Phantom requires missing dependencies: {}. {}",
            names.join(", "),
            hints.join(" ")
        ),
    )
}

pub(crate) fn phantom_preflight(pm: Option<platform::PackageManager>) -> Result<(), AppError> {
    let (packages, sdk) = phantom_missing_dependencies();
    if packages.is_empty() && sdk.is_empty() {
        return Ok(());
    }
    Err(phantom_dependency_error(&packages, &sdk, pm))
}

pub(crate) fn phantom_disk_preflight() -> Result<(), AppError> {
    let home = std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| {
        AppError::not_found("HOME", "Cannot determine the desktop home directory.")
    })?;
    if let Some(kb) = check_available_disk_space_kb(&home) {
        if kb < 1_000_000 {
            return Err(AppError::command_failed(format!(
                "Insufficient disk space in {}: only {:.0} MB available. Phantom needs about 1 GB to build its Rust binaries and Android server jar.",
                home.display(),
                kb as f64 / 1024.0
            )));
        }
    }
    Ok(())
}
