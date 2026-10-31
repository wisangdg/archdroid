# ArchDroid

ArchDroid adalah aplikasi desktop berbasis **Tauri v2 + React 18** untuk mengelola [Waydroid](https://waydroid.net/) di Linux dengan gaya modern gaming glassmorphism (mirip LDPlayer / NoxPlayer).

ArchDroid mempermudah inisialisasi image Android, manajemen aplikasi APK, injeksi GApps & ARM translation (libhoudini), kontrol resolusi/DPI, keymapping virtual touchscreen (Phantom), serta diagnostik host Linux dalam satu antarmuka desktop.

---

## 1. Distribusi Linux yang Didukung (Platform Support Contract)

ArchDroid dirancang dengan arsitektur **multi-distro** yang secara otomatis mendeteksi package manager host dan memetakan paket dependency yang sesuai.

### Target Resmi yang Didukung (Supported)

| Distribusi | Package Manager | Format Paket | Status Dukungan |
|---|---|---|---|
| **Arch Linux / CachyOS / EndeavourOS / Manjaro** | `pacman` | AUR / Source / AppImage | **Didukung Penuh (Tier 1)** |
| **Ubuntu / Debian / Linux Mint / Pop!_OS** | `apt-get` | `.deb` / AppImage | **Didukung Penuh (Tier 1)** |
| **Fedora / RHEL / Alma / Rocky Linux** | `dnf` | RPM / AppImage | **Didukung (Tier 1)** |
| **openSUSE (Tumbleweed / Leap)** | `zypper` | RPM / AppImage | **Didukung (Tier 1)** |

> [!NOTE]
> Dependency sistem (`waydroid`, `squashfs-tools` / `squashfs`, `lzip`, `git`) dipetakan secara dinamis berdasarkan package manager host yang terdeteksi, dan diinstal menggunakan flag non-interaktif minimalis (`--needed` / `--no-install-recommends`).

### Target yang Tidak Didukung (Explicitly Unsupported)

- **Sesi Desktop Murni X11 (Pure X11)**:
  Waydroid membutuhkan Wayland compositor (`WAYLAND_DISPLAY`). Desktop environment yang didukung mencakup GNOME Wayland, KDE Plasma Wayland, Hyprland, Sway, dan compositor Wayland lainnya. Sesi X11 murni **tidak didukung** secara langsung (memerlukan nested compositor seperti Weston/cage).
- **Distribusi Non-Systemd / Non-Standard Package Manager**:
  Distribusi seperti Alpine Linux (`apk`), Void Linux (`xbps`), NixOS, atau GNU Guix **tidak didukung untuk instalasi dependensi otomatis**. Pada platform ini, installer dependensi akan **gagal cepat (fast-fail) sebelum meminta root**, dan pengguna harus memasang `waydroid`, `squashfs-tools`, dan `lzip` secara manual melalui repositori distro masing-masing.
- **Sistem Operasi Non-Linux**:
  Windows dan macOS **tidak didukung** (Waydroid bergantung pada Linux kernel namespaces, cgroups, dan binder IPC).

---

## 2. Kebutuhan Sistem Host (Prerequisites)

1. **Kernel Linux**: Versi 5.x atau lebih baru dengan modul `binder` dan `ashmem` (atau `binderfs`). Kernel bawaan Arch, Ubuntu, Fedora, dan openSUSE modern sudah menyertakan modul ini.
2. **Akselerasi Grafis (GPU / DRI)**: Node render `/dev/dri/renderD128` dengan driver Mesa/Vulkan.
3. **Wayland Compositor**: Sesi desktop Wayland aktif (`WAYLAND_DISPLAY`).
4. **Agen Otentikasi Polkit atau Sudo**:
   - Untuk GUI desktop: Agen Polkit aktif (misalnya `hyprpolkitagent`, `polkit-gnome`, atau `polkit-kde-agent`).
   - Atau jalankan aplikasi dari terminal jika menggunakan fallback sudo.

---

## 3. Fitur Utama

- 🎮 **App Library & Game Launcher**: Daftar aplikasi Android yang terpasang dengan ekstraksi ikon, pencarian instan (`Ctrl+F`), filter kategori (All/User/System), dan klik kanan context menu.
- 📦 **Drag-and-Drop APK Installer**: Tarik file `.apk` dari file manager langsung ke jendela ArchDroid untuk instalasi instan.
- 🧩 **Add-on Injection (GApps & ARM Translation)**:
  - Injeksi Google Play Services (MindTheGapps / OpenGApps).
  - Injeksi ARM translation layer (Intel libhoudini) untuk memainkan game Android 3D mobile pada CPU x86_64.
  - Deklarasi hardware multitouch untuk kompatibilitas penuh Google Play Store.
  - Setiap injeksi root dilindungi oleh **dialog persetujuan eksplisit** sebelum meminta akses polkit/sudo.
- 🖥️ **Display Profiles & Resolution Presets**: Pilihan preset cepat (Tablet 1080p, Tablet 720p, Mobile Portrait, Gaming 2K) dan kustomisasi DPI dinamis.
- ⌨️ **Smart Keymapping (Phantom Keymapper)**: Integrasi keymapper virtual touchscreen desktop unprivileged.
- 🩺 **Host System Diagnostics**: Pengecekan otomatis terhadap KVM, binder, GPU render node, sesi Wayland, daemon Waydroid, dan storage.
- 📟 **Visual Process Stepper & Terminal**: Stepper visual interaktif dengan log parser multi-distro, kemampuan ekspor log (`.log`), pembatalan task (`cancel`), dan retry.

---

## 4. Keamanan & Privilege Boundary

ArchDroid menerapkan prinsip keamanan defensif berlapis:
- **Hash-Pinned Python Dependencies**: Dependensi Python untuk helper instalasi di-lock dan diverifikasi dengan hash SHA-256 (`--require-hashes --only-binary=:all:`).
- **Isolasi Lingkungan Root**: Script privileged berjalan di direktori temporer privat (`mktemp -d`, mode `0700`), `SUDO_USER` di-unset, dan cache diarahkan ke `$ARCHDROID_WORK/cache` agar proses root tidak pernah menulis file ke `~/.cache` milik user desktop.
- **Explicit Consent**: Semua installer root wajib dikonfirmasi melalui modal persetujuan pengguna sebelum prompt root polkit/sudo muncul.
- **Fail-Fast Preflight**: Jika dependensi hilang pada distribusi yang tidak didukung, sistem menolak operasi sebelum meminta izin root.

---

## 5. Pengembangan & Build

### Menjalankan Mode Development
```bash
# Instal dependensi frontend
npm install

# Jalankan Vite dev server + Tauri IPC
npm run tauri dev
```

### Menjalankan Pengujian (Testing)
```bash
# Unit test frontend & security assertions
npm test

# Unit test backend Rust (multi-distro, elevation, parser)
cargo test --manifest-path src-tauri/Cargo.toml

# Test dengan fitur dev-commands
cargo test --manifest-path src-tauri/Cargo.toml --features dev-commands
```

### Membangun Paket Rilis (Production Build)
```bash
npm run tauri build
```
Hasil paket rilis (`.deb`, `.AppImage`) akan dibuat di folder `src-tauri/target/release/bundle/`.

---

## 6. Lisensi & Upstream Credits

- **ArchDroid**: Lisensi MIT / GPL.
- **Waydroid**: [https://waydroid.net](https://waydroid.net) (GPL-3.0).
- **waydroid_script**: [casualsnek/waydroid_script](https://github.com/casualsnek/waydroid_script) (GPL-3.0) — Pinned commit `48dbfaf`.
- **Phantom Keymapper**: [oliviermugishak/phantom](https://github.com/oliviermugishak/phantom) (MIT) — Pinned commit `1a6b7ed`.
Lihat [`docs/THIRD-PARTY.md`](docs/THIRD-PARTY.md) untuk rincian audit pihak ketiga.
