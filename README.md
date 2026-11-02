# ArchDroid

ArchDroid is a **Tauri v2 + React 18** desktop app for managing [Waydroid](https://waydroid.net/) on Linux, with a modern glassmorphism interface (inspired by desktop Android emulators such as LDPlayer / NoxPlayer).

ArchDroid brings Android image initialization, APK app management, GApps & ARM translation (libhoudini) injection, resolution/DPI control, virtual touchscreen keymapping (Phantom), and Linux host diagnostics together in one unified, safe desktop interface.

---

## 1. Supported Linux Distributions (Platform Support Contract)

ArchDroid uses a **multi-distro** architecture that detects the host package manager automatically and maps the matching dependency packages.

### Official Target — Tier 1 (Fully Supported & Live-Tested)

| Distribution | Package Manager | Package Format | Support Status & Test Evidence |
|---|---|---|---|
| **Arch Linux / CachyOS / EndeavourOS / Manjaro** | `pacman` | Local PKGBUILD (`makepkg -si`) / AppImage | **Fully Supported (Tier 1)**<br>Live-tested in an Arch Linux QEMU/KVM VM (kernel 7.2.11, `gtk-launch` desktop launcher, Waydroid daemon, preflight, install/uninstall). See [docs/P0-8.2-MATRIX-RESULTS.md](docs/P0-8.2-MATRIX-RESULTS.md) and [docs/P0-9.1-PACKAGING-RESULTS.md](docs/P0-9.1-PACKAGING-RESULTS.md). |

### Experimental Targets — Tier 2 (Best-Effort)

| Distribution | Package Manager | Package Format | Support Status |
|---|---|---|---|
| **Ubuntu / Debian / Linux Mint / Pop!_OS** | `apt-get` | Preview `.deb` / AppImage | **Tier 2 (Best-Effort)**<br>`.deb` live-tested in an Ubuntu 24.04 LTS VM: install, setup, Waydroid installed by the app through `apt` (after adding the official Waydroid repo), init via polkit, engine start/stop, APK install via drag-and-drop, CSP. Other variants (Debian, Mint, Pop!_OS) are untested. |
| **Fedora / RHEL / Alma / Rocky Linux** | `dnf` | Preview `.rpm` / AppImage | **Tier 2 (Best-Effort)**<br>`.rpm` live-tested in a Fedora 44 VM: `dnf install` (pulls `waydroid` from the Fedora repos), init via polkit, engine start/stop, app list, APK install via drag-and-drop, CSP, `dnf remove arch-droid`. RHEL/Alma/Rocky are untested. |
| **openSUSE (Tumbleweed / Leap)** | `zypper` | Preview `.rpm` / AppImage | **Tier 2 (Best-Effort)**<br>The package mapping contract is covered by backend unit tests (`platform.rs`), but has not gone through the full live OS matrix. |

> [!NOTE]
> System dependencies (`waydroid`, `squashfs-tools` / `squashfs`, `lzip`, `git`) are mapped dynamically from the detected host package manager and installed with minimal non-interactive flags (`--needed` / `--no-install-recommends`). ArchDroid never installs packages silently: the confirmation dialog lists the packages and the exact command that will run, and the root script only installs the approved packages.

### Artifact Integrity & Verification

Every official Tier 1 release (`archdroid-bin-*.pkg.tar.zst` and `ArchDroid_*.AppImage`) ships with a `SHA256SUMS` checksum file and a GPG signature `SHA256SUMS.asc` from the official public key (`18F47067ACC5BBF9`). Manual and automated verification steps are in [docs/VERIFY-RELEASE.md](docs/VERIFY-RELEASE.md). Update strategy (manual, no updater) and key management: [docs/RELEASE-AND-UPDATES.md](docs/RELEASE-AND-UPDATES.md).

### Unsupported Targets (Explicitly Unsupported)

- **Pure X11 Desktop Sessions**:
  Waydroid needs a Wayland compositor (`WAYLAND_DISPLAY`). Supported desktop environments include GNOME Wayland, KDE Plasma Wayland, Hyprland, Sway, and other Wayland compositors. Pure X11 sessions are **not supported** directly (they need a nested compositor such as Weston/cage).
- **Non-systemd / Non-standard Package Manager Distributions**:
  Distributions such as Alpine Linux (`apk`), Void Linux (`xbps`), NixOS, or GNU Guix are **not supported for automatic dependency installation**. On these platforms the dependency installer **fails fast before asking for root**, and users must install `waydroid`, `squashfs-tools`, and `lzip` manually from their distro repositories.
- **Non-Linux Operating Systems**:
  Windows and macOS are **not supported** (Waydroid depends on Linux kernel namespaces, cgroups, and binder IPC).

---

## 2. Host System Requirements (Prerequisites)

1. **Linux Kernel**: 5.x or newer with the `binder` and `ashmem` kernel modules (or `binderfs`). Stock kernels on modern Arch, Ubuntu, Fedora, and openSUSE already include them.
2. **Graphics Acceleration (GPU / DRI)**: A `/dev/dri/renderD128` render node with Mesa/Vulkan drivers.
3. **Wayland Compositor**: An active Wayland desktop session (`WAYLAND_DISPLAY` set).
4. **Polkit Authentication Agent or Sudo**:
   - For desktop GUIs: a Polkit agent running in your desktop session (for example `hyprpolkitagent`, `polkit-gnome`, or `polkit-kde-agent`).
   - Or the terminal sudo fallback when no graphical Polkit agent is available.

### Dependency Preflight

Before any action that needs a dependency or root, ArchDroid checks the host and shows the missing items with the command that fixes them:

- **Setup Wizard**: Shows the host diagnostics before the initialize button is enabled. Elevation, network, or disk problems, or a missing `waydroid` with no supported package manager, keep the init button disabled.
- **Settings → Host Diagnostics**: Shows 17 checks, including `waydroid`, `adb`, Wayland, `/dev/kvm`, binder, `/dev/dri`, polkit/sudo, `git`, `python3` + `python3-venv`, `unsquashfs`, `lzip`, `xdg-open`, terminal emulator, disk space, internet connection, and Android image status.
- **GApps/Houdini/Multitouch Installers**: Fail fast before the root prompt when elevation, network, disk, or the Waydroid image is not ready. The GApps/Houdini root scripts install the supporting dependencies the user approved in the confirmation dialog before checkout.

---

## 3. Installation

### Arch Linux / Arch-based Distributions (Tier 1)

**Option A: Local PKGBUILD (Recommended)**
```bash
git clone https://github.com/wisangdg/archdroid.git
cd archdroid/packaging/arch
makepkg -si
```

**Option B: Release Binary Package (`.pkg.tar.zst`)**
Download the `archdroid-bin-<version>-x86_64.pkg.tar.zst` artifact, verify its signature, then install it with pacman:
```bash
sudo pacman -U archdroid-bin-*.pkg.tar.zst
```

### Standalone Executable (AppImage)

The ArchDroid AppImage runs directly on any x86_64 Linux distribution with Wayland:
```bash
chmod +x ArchDroid_*.AppImage
./ArchDroid_*.AppImage
```
> [!TIP]
> ArchDroid clears the AppImage's internal `PYTHONHOME` and `PYTHONPATH` environment variables when it calls host system tools (`/usr/bin/waydroid`), which prevents Python module crashes on the host distro.

### Other Distributions (Tier 2 Preview)

- **Debian / Ubuntu**: Ubuntu 24.04 has no `waydroid` package in its official repos; add the [official Waydroid repo](https://docs.waydro.id/usage/install-on-desktops) first, then install the preview package with apt:
  ```bash
  sudo apt install ./ArchDroid_*_amd64.deb
  ```
- **Fedora / openSUSE**: Install the preview package with dnf/zypper:
  ```bash
  sudo dnf install ./ArchDroid-*.x86_64.rpm
  ```

---

## 4. First Run & Setup Wizard

When ArchDroid opens for the first time:

1. **Waydroid Status Detection**:
   ArchDroid checks whether Waydroid is installed and whether the Android system image has been initialized in `/var/lib/waydroid`.
2. **Interactive Setup Wizard**:
   If there is no Android image yet, the **Setup Wizard** greets you:
   - **Host Health Preflight**: Checks the kernel binder, GPU DRI, Wayland session, and disk space (at least 10 GB recommended).
   - **Android Image Type**:
     - `VANILLA`: Plain Android (lightweight, open source, no Google services).
     - `GAPPS`: Android with Google Mobile Services and the Google Play Store.
   - **Automatic Download & Extract**: Shows a progress bar and a live log while the Android image is downloaded and extracted on the host.
3. **Start the Waydroid Service**:
   Once finished, the Waydroid container session is ready to start and the App Library loads the built-in Android apps.

---

## 5. Privilege Model & Root Safety (Privilege Boundaries)

ArchDroid follows the **least-privilege** principle:

### When Is Root Needed?
Root access (through graphical Polkit or sudo) is requested **only** for these system administration operations:
1. **Installing System Dependencies**: Installing missing packages (`waydroid`, `squashfs-tools`, `lzip`, `git`) through the host package manager.
2. **First-Time Initialization (`waydroid init`)**: Setting up the `/var/lib/waydroid` mount namespace and the Android image files as root.
3. **System Add-on Injection (`GApps`, `libhoudini`, `multitouch`)**: Modifying the Waydroid system partition (`/var/lib/waydroid/images/system.img`).

### When Is Root NEVER Requested?
Day-to-day use of ArchDroid runs **100% unprivileged (as a normal user)**:
- Opening and running ArchDroid.
- Starting the Waydroid session (`waydroid session start`).
- Launching Android apps and games.
- Installing `.apk` files (drag-and-drop or file picker).
- Changing screen resolution and DPI profiles.
- Configuring and running Phantom Smart Keymapping.
- Running host diagnostics.

### Explicit Consent Dialog
Before any root password prompt (Polkit/sudo) appears, ArchDroid **always shows a confirmation modal** that:
- Explains the exact command that will run.
- Lists the distro system packages that will be installed.
- The Rust backend re-checks that the packages being installed exactly match what the user approved (rejecting injected packages).
- Root scripts run in a private temporary directory (`mode 0700`) with an isolated cache so they never pollute the user's directories.

---

## 6. Troubleshooting

### 1. Root Dialog Does Not Appear on Standalone Window Managers (Hyprland / Sway / i3)
- **Symptom**: The dependency install or init button hangs/times out with no password popup.
- **Cause**: Standalone window managers do not ship a Polkit authentication agent by default.
- **Fix**: Install a polkit agent and start it with your session:
  ```bash
  # For Hyprland
  systemctl --user enable --now hyprpolkitagent.service
  # Or start polkit-gnome / polkit-kde from autostart
  /usr/lib/polkit-gnome/polkit-gnome-authentication-agent-1 &
  ```
  Or launch `archdroid` from a terminal to get the text-based `sudo` password prompt.

### 2. Missing Binder Kernel Module (`binder: missing`)
- **Symptom**: Diagnostics show Binder missing; Waydroid cannot start the container.
- **Cause**: The running Linux kernel does not enable the `binder_linux` module or binderfs.
- **Fix**:
  - On Arch Linux: use the stock `linux` or `linux-zen` kernel, which enable binder by default.
  - On a custom kernel, make sure these options are enabled:
    ```ini
    CONFIG_ANDROID_BINDER_IPC=y
    CONFIG_ANDROID_BINDERFS=y
    ```
  - Make sure binderfs is mounted:
    ```bash
    sudo mkdir -p /dev/binderfs
    sudo mount -t binder binder /dev/binderfs
    ```

### 3. Wayland Compositor Not Detected (`Wayland: not running`)
- **Symptom**: The app warns that no Wayland session was found.
- **Cause**: You are logged into a pure X11 desktop session (`DISPLAY` set, but `WAYLAND_DISPLAY` empty).
- **Fix**: Log into a Wayland session (choose "GNOME on Wayland", "Plasma (Wayland)", or run a compositor such as Hyprland/Sway). To run under an X11 session, use a nested Wayland compositor such as `weston` or `cage`.

### 4. KVM Permission Denied (`/dev/kvm: permission denied`)
- **Symptom**: Slow emulation or a KVM permission warning.
- **Fix**: Add your user to the `kvm` group:
  ```bash
  sudo usermod -aG kvm $USER
  ```
  Then log out and log back in.

### 5. GPU Compatibility (Nvidia vs AMD / Intel)
- **Intel & AMD**: Work out of the box with the open-source Mesa drivers (DRI3/Vulkan).
- **Nvidia Proprietary**: Make sure `nvidia-drm.modeset=1` is set in the kernel boot parameters. If Waydroid shows render glitches, it can be configured to fall back to the software renderer (`Mesa llvmpipe`).

### 6. Reporting Problems (Diagnostic Bundle)
Open **Settings → Diagnostic Bundle → Create Bundle**. ArchDroid writes one text file to `~/Downloads/archdroid-diagnostics-<time>.txt` with the system and Waydroid versions, host diagnostics, add-on status, and recent logs, with home paths and token/password values masked. The logs can contain Android package names and APK file names you have used; read the file and remove anything you do not want to share, then attach it to the bug report. The file is created without root and is never uploaded automatically.

---

## 7. Clean Uninstall

To remove ArchDroid and all of its data from your system:

### 1. Remove the ArchDroid App
- **Arch Linux**:
  ```bash
  sudo pacman -R archdroid-bin
  ```
- **Debian / Ubuntu**:
  ```bash
  sudo apt remove arch-droid
  ```
- **Fedora / openSUSE**:
  ```bash
  sudo dnf remove arch-droid   # openSUSE: sudo zypper remove arch-droid
  ```
- **AppImage**: Delete the `ArchDroid_*.AppImage` file and its `.desktop` file in `~/.local/share/applications/` if there is one.

### 2. Remove User Configuration & Cache
Remove local preferences, Phantom keymaps, and the temporary cache:
```bash
rm -rf ~/.config/archdroid
rm -rf ~/.cache/archdroid
```

### 3. Remove Waydroid Data & Images (Optional)
To also remove the whole Android system, the apps installed inside the container, and the Waydroid images:
```bash
# Stop the Waydroid session and service
waydroid session stop
sudo systemctl stop waydroid-container.service

# Remove system data and container images (requires root)
sudo rm -rf /var/lib/waydroid

# Remove user runtime data
rm -rf ~/.local/share/waydroid
```

---

## 8. Known Limitations

1. **One Container per Host (Single Instance Container)**:
   By design, Waydroid uses one global LXC container per host machine. ArchDroid does not support running multiple Android containers in parallel.
2. **Wayland Session Dependency**:
   ArchDroid and Waydroid need an active Wayland compositor. The app has no built-in isolated X11 display server.
3. **ARM Translation Layer (libhoudini) Availability**:
   libhoudini injection depends on the upstream Google Android TV image downloaded by a pinned helper script. Games with kernel-level anti-cheat (such as some heavily protected titles) may not run in an emulator container.
4. **Distro Support Levels**:
   Full (Tier 1) live-verified support covers the Arch Linux family. Tier 2 distros are in preview: the `.deb` is live-tested on Ubuntu 24.04 LTS and the `.rpm` on Fedora 44, while Debian, the RHEL family, and openSUSE are not live-tested yet.
5. **Live Test Coverage**:
   All main flows, including building and launching Phantom and Wayland and X11 drag-and-drop, are live-tested in an Arch VM (weston and Xvfb; see [`docs/P0-8.2-MATRIX-RESULTS.md`](docs/P0-8.2-MATRIX-RESULTS.md)). Not yet tested: the GPU and desktop matrix on real hardware (GNOME/KDE/Hyprland, Intel/AMD/NVIDIA). If something fails on your system, attach a Diagnostic Bundle to your report.
6. **Add-on Archive Integrity**:
   The `waydroid_script` script is pinned to a specific commit and its Python dependencies are SHA-256 verified, but the GApps/libhoudini archives it downloads are checked against the **MD5** hashes embedded in that commit. MD5 catches corrupted or swapped files, but it is not strong cryptographic protection.
7. **Connectivity Probe**:
   The "online" check opens a TCP connection, sending no data, to `1.1.1.1:53` (then `8.8.8.8:53`). On networks that block public DNS, diagnostics can show "Offline" even though downloads from upstream still work. Details are in [PRIVACY.md](PRIVACY.md).

---

## 9. Key Features

- 🎮 **App Library & Game Launcher**: A list of installed Android apps with icon extraction, instant search (`Ctrl+F`), category filters (All/User/System), and a right-click context menu.
- 📦 **Drag-and-Drop APK Installer**: Drag an `.apk` file from your file manager straight onto the ArchDroid window to install it.
- 🧩 **Add-on Injection (GApps & ARM Translation)**:
  - Google Play Services injection (MindTheGapps / OpenGApps).
  - ARM translation layer injection (Intel libhoudini) to play 3D Android mobile games on x86_64 CPUs.
  - Multitouch hardware declaration for full Google Play Store compatibility.
  - Every root injection is guarded by an **explicit consent dialog** before polkit/sudo access is requested.
- 🖥️ **Display Profiles & Resolution Presets**: Quick presets (Tablet 1080p, Tablet 720p, Mobile Portrait, Gaming 2K) and dynamic DPI customization.
- ⌨️ **Smart Keymapping (Phantom Keymapper)**: Integration with an unprivileged desktop virtual-touchscreen keymapper.
- 🩺 **Host System Diagnostics**: Automatic checks for KVM, binder, the GPU render node, the Wayland session, the Waydroid daemon, and storage.
- 📟 **Visual Process Stepper & Terminal**: An interactive visual stepper with a multi-distro log parser, log export (`.log`), task cancellation (`cancel`), and retry.

---

## 10. Security & Privilege Boundary

ArchDroid applies layered defensive security:
- **Hash-Pinned Python Dependencies**: Python dependencies for the install helpers are locked and verified by SHA-256 hash (`--require-hashes --only-binary=:all:`).
- **Root Environment Isolation**: Privileged scripts run in a private temporary directory (`mktemp -d`, mode `0700`), `SUDO_USER` is unset, and the cache points to `$ARCHDROID_WORK/cache`, so root processes never write to the desktop user's `~/.cache`.
- **Explicit Consent**: Every root installer must be confirmed in a user consent modal before the polkit/sudo root prompt appears.
- **Package Approval**: The modal shows the system packages and the exact command that will run; the backend re-validates the approved list and refuses to install packages outside the plan (`approval_required` / `invalid_input`).
- **Fail-Fast Preflight**: If dependencies are missing on an unsupported distribution, the operation is refused before root is requested.

---

## 11. Development & Build

### Development Mode
```bash
# Install frontend dependencies
npm install

# Start the Vite dev server + Tauri IPC
npm run tauri dev
```

### Running Tests
```bash
# Frontend unit tests & security assertions
npm test

# Rust backend unit tests (multi-distro, elevation, parser)
cargo test --manifest-path src-tauri/Cargo.toml

# Tests with the dev-commands feature
cargo test --manifest-path src-tauri/Cargo.toml --features dev-commands
```

### Production Build
```bash
npm run tauri build
```
The release packages (`.deb`, `.AppImage`) are written to `src-tauri/target/release/bundle/`.

---

## 12. License & Upstream Credits

- **ArchDroid**: [MIT](LICENSE) license, Copyright (c) 2026 wisangdg.
- **Waydroid**: [https://waydroid.net](https://waydroid.net) (GPL-3.0).
- **waydroid_script**: [casualsnek/waydroid_script](https://github.com/casualsnek/waydroid_script) (GPL-3.0) — pinned commit `48dbfaf`. The script is downloaded when an add-on is installed and is not vendored in the ArchDroid binary.
- **Phantom Keymapper**: [oliviermugishak/phantom](https://github.com/oliviermugishak/phantom) (MIT) — pinned commit `1a6b7ed`.
- Third-party details and the license audit are in [`docs/THIRD-PARTY.md`](docs/THIRD-PARTY.md).

> [!NOTE]
> **Trademark Disclaimer**: LDPlayer and NoxPlayer are trademarks or registered trademarks of their respective owners. These names are used in this documentation only as a user-interface comparison reference. ArchDroid is an independent project and is not affiliated with, sponsored by, or endorsed by those trademark holders.
