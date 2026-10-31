# ArchDroid

ArchDroid adalah aplikasi desktop berbasis **Tauri v2 + React 18** untuk mengelola [Waydroid](https://waydroid.net/) di Linux dengan antarmuka modern glassmorphism (terinspirasi dari emulator Android desktop seperti LDPlayer / NoxPlayer).

ArchDroid mempermudah inisialisasi image Android, manajemen aplikasi APK, injeksi GApps & ARM translation (libhoudini), kontrol resolusi/DPI, keymapping virtual touchscreen (Phantom), serta diagnostik host Linux dalam satu antarmuka desktop yang terpadu dan aman.

---

## 1. Distribusi Linux yang Didukung (Platform Support Contract)

ArchDroid dirancang dengan arsitektur **multi-distro** yang secara otomatis mendeteksi package manager host dan memetakan paket dependency yang sesuai.

### Target Resmi — Tier 1 (Didukung Penuh & Teruji Live)

| Distribusi | Package Manager | Format Paket | Status Dukungan & Bukti Uji |
|---|---|---|---|
| **Arch Linux / CachyOS / EndeavourOS / Manjaro** | `pacman` | PKGBUILD lokal (`makepkg -si`) / AppImage | **Didukung Penuh (Tier 1)**<br>Teruji live di VM QEMU/KVM Arch Linux (kernel 7.2.11, desktop launcher `gtk-launch`, Waydroid daemon, preflight, install/uninstall). Lihat [docs/P0-8.2-MATRIX-RESULTS.md](docs/P0-8.2-MATRIX-RESULTS.md) dan [docs/P0-9.1-PACKAGING-RESULTS.md](docs/P0-9.1-PACKAGING-RESULTS.md). |

### Target Eksperimental — Tier 2 (Best-Effort — Belum Diuji Live)

| Distribusi | Package Manager | Format Paket | Status Dukungan |
|---|---|---|---|
| **Ubuntu / Debian / Linux Mint / Pop!_OS** | `apt-get` | Preview `.deb` / AppImage | **Tier 2 (Best-Effort)**<br>Kontrak mapping paket teruji di unit test backend (`platform.rs`), tetapi belum melalui live testing matriks sistem operasi penuh. |
| **Fedora / RHEL / Alma / Rocky Linux** | `dnf` | Preview `.rpm` / AppImage | **Tier 2 (Best-Effort)**<br>Kontrak mapping paket teruji di unit test backend (`platform.rs`), tetapi belum melalui live testing matriks sistem operasi penuh. |
| **openSUSE (Tumbleweed / Leap)** | `zypper` | Preview `.rpm` / AppImage | **Tier 2 (Best-Effort)**<br>Kontrak mapping paket teruji di unit test backend (`platform.rs`), tetapi belum melalui live testing matriks sistem operasi penuh. |

> [!NOTE]
> Dependency sistem (`waydroid`, `squashfs-tools` / `squashfs`, `lzip`, `git`) dipetakan secara dinamis berdasarkan package manager host yang terdeteksi, dan diinstal menggunakan flag non-interaktif minimalis (`--needed` / `--no-install-recommends`). ArchDroid tidak pernah memasang paket secara diam-diam: dialog konfirmasi menampilkan daftar paket dan command persis yang akan dijalankan, dan script root hanya menjalankan paket yang disetujui.

### Integritas & Verifikasi Artefak

Setiap rilis resmi Tier 1 (`archdroid-bin-*.pkg.tar.zst` dan `ArchDroid_*.AppImage`) disertai file checksum `SHA256SUMS` dan tanda tangan digital GPG `SHA256SUMS.asc` dari kunci publik resmi (`18F47067ACC5BBF9`). Panduan verifikasi manual dan otomatis dapat dilihat di [docs/VERIFY-RELEASE.md](docs/VERIFY-RELEASE.md). Strategi update (manual, tanpa updater) dan pengelolaan kunci: [docs/RELEASE-AND-UPDATES.md](docs/RELEASE-AND-UPDATES.md).

### Target yang Tidak Didukung (Explicitly Unsupported)

- **Sesi Desktop Murni X11 (Pure X11)**:
  Waydroid membutuhkan Wayland compositor (`WAYLAND_DISPLAY`). Desktop environment yang didukung mencakup GNOME Wayland, KDE Plasma Wayland, Hyprland, Sway, dan compositor Wayland lainnya. Sesi X11 murni **tidak didukung** secara langsung (memerlukan nested compositor seperti Weston/cage).
- **Distribusi Non-Systemd / Non-Standard Package Manager**:
  Distribusi seperti Alpine Linux (`apk`), Void Linux (`xbps`), NixOS, atau GNU Guix **tidak didukung untuk instalasi dependensi otomatis**. Pada platform ini, installer dependensi akan **gagal cepat (fast-fail) sebelum meminta root**, dan pengguna harus memasang `waydroid`, `squashfs-tools`, dan `lzip` secara manual melalui repositori distro masing-masing.
- **Sistem Operasi Non-Linux**:
  Windows dan macOS **tidak didukung** (Waydroid bergantung pada Linux kernel namespaces, cgroups, dan binder IPC).

---

## 2. Kebutuhan Sistem Host (Prerequisites)

1. **Kernel Linux**: Versi 5.x atau lebih baru dengan modul kernel `binder` dan `ashmem` (atau `binderfs`). Kernel bawaan Arch, Ubuntu, Fedora, dan openSUSE modern sudah menyertakan modul ini.
2. **Akselerasi Grafis (GPU / DRI)**: Node render `/dev/dri/renderD128` dengan driver Mesa/Vulkan.
3. **Wayland Compositor**: Sesi desktop Wayland aktif (`WAYLAND_DISPLAY` terdefinisi).
4. **Agen Otentikasi Polkit atau Sudo**:
   - Untuk GUI desktop: Agen Polkit aktif di sesi desktop Anda (misalnya `hyprpolkitagent`, `polkit-gnome`, atau `polkit-kde-agent`).
   - Atau fallback terminal sudo jika agen Polkit grafis tidak tersedia.

### Preflight Dependency

Sebelum aksi yang membutuhkan dependency atau root, ArchDroid memeriksa host dan menampilkan item yang kurang beserta perintah perbaikannya:

- **Setup Wizard**: Menampilkan hasil diagnosa sistem host sebelum tombol inisialisasi aktif. Item elevation, jaringan, disk, atau ketiadaan `waydroid` tanpa package manager yang didukung akan menahan tombol init.
- **Settings → Host Diagnostics**: Menampilkan 17 pemeriksaan menyeluruh, termasuk `waydroid`, `adb`, Wayland, `/dev/kvm`, binder, `/dev/dri`, polkit/sudo, `git`, `python3` + `python3-venv`, `unsquashfs`, `lzip`, `xdg-open`, terminal emulator, ruang disk, koneksi internet, dan status image Android.
- **Installer GApps/Houdini/Multitouch**: Gagal cepat (*fast-fail*) sebelum prompt root bila elevation, jaringan, disk, atau image Waydroid belum siap. Script root GApps/Houdini memasang dependensi pendukung yang disetujui pengguna di dialog konfirmasi sebelum checkout.

---

## 3. Panduan Instalasi (Installation)

### Arch Linux / Distribusi Berbasis Arch (Tier 1)

**Opsi A: Menggunakan PKGBUILD Lokal (Direkomendasikan)**
```bash
git clone https://github.com/wisangdg/archdroid.git
cd archdroid/packaging/arch
makepkg -si
```

**Opsi B: Menggunakan Paket Binary Rilis (`.pkg.tar.zst`)**
Unduh artefak `archdroid-bin-<versi>-x86_64.pkg.tar.zst` dan verifikasi tanda tangannya, lalu pasang dengan pacman:
```bash
sudo pacman -U archdroid-bin-*.pkg.tar.zst
```

### Standalone Executable (AppImage)

ArchDroid AppImage dapat langsung dijalankan di semua distribusi Linux x86_64 dengan Wayland:
```bash
chmod +x ArchDroid_*.AppImage
./ArchDroid_*.AppImage
```
> [!TIP]
> ArchDroid secara otomatis membersihkan variabel environment `PYTHONHOME` dan `PYTHONPATH` internal AppImage saat memanggil tool sistem host (`/usr/bin/waydroid`), mencegah crash modul Python pada distro host.

### Distribusi Lain (Preview Tier 2)

- **Debian / Ubuntu**: Pasang paket preview dengan apt:
  ```bash
  sudo apt install ./archdroid_*_amd64.deb
  ```
- **Fedora / openSUSE**: Pasang paket preview dengan dnf/zypper:
  ```bash
  sudo dnf install ./archdroid-*.x86_64.rpm
  ```

---

## 4. First Run & Setup Wizard

Saat ArchDroid dibuka untuk pertama kali:

1. **Deteksi Status Waydroid**:
   ArchDroid otomatis memeriksa apakah Waydroid sudah terpasang dan apakah image sistem Android sudah diinisialisasi di `/var/lib/waydroid`.
2. **Setup Wizard Interaktif**:
   Jika image Android belum ada, jendela **Setup Wizard** akan menyambut Anda:
   - **Host Health Preflight**: Memeriksa kesiapan kernel binder, GPU DRI, sesi Wayland, dan ruang penyimpanan disk (minimal 10 GB direkomendasikan).
   - **Tipe Image Android**:
     - `VANILLA`: Sistem Android murni (ringan, open-source, tanpa layanan Google).
     - `GAPPS`: Android yang sudah dilengkapi Google Mobile Services dan Google Play Store.
   - **Download & Ekstrak Otomatis**: Menampilkan progress bar dan log langsung saat image Android diunduh dan diekstrak ke host.
3. **Mulai Layanan Waydroid**:
   Setelah selesai, sesi container Waydroid akan siap dijalankan dan App Library akan memuat aplikasi bawaan Android.

---

## 5. Model Hak Akses & Keamanan Root (Privilege Boundaries)

ArchDroid dirancang dengan prinsip **least-privilege** (hak akses seminimal mungkin):

### Kapan Root Dibutuhkan?
Akses root (via Polkit grafis atau sudo) **hanya** diminta untuk operasi administratif sistem berikut:
1. **Instalasi Paket Dependensi Sistem**: Memasang paket yang belum ada (`waydroid`, `squashfs-tools`, `lzip`, `git`) melalui package manager host.
2. **Inisialisasi Pertama Kali (`waydroid init`)**: Menyiapkan mount namespace `/var/lib/waydroid` dan file image Android di level root.
3. **Injeksi Add-on Sistem (`GApps`, `libhoudini`, `multitouch`)**: Memodifikasi partisi sistem Waydroid (`/var/lib/waydroid/images/system.img`).

### Kapan Root TIDAK PERNAH Diminta?
Operasional harian ArchDroid berjalan **100% unprivileged (sebagai user biasa)** tanpa izin root:
- Membuka dan menjalankan ArchDroid.
- Menjalankan sesi Waydroid (`waydroid session start`).
- Meluncurkan aplikasi dan game Android.
- Memasang file `.apk` (drag-and-drop atau file picker).
- Mengubah profil resolusi dan DPI layar.
- Mengonfigurasi dan menjalankan Smart Keymapping Phantom.
- Menjalankan pemeriksaan diagnostik host.

### Dialog Persetujuan Eksplisit (Explicit Consent)
Sebelum prompt password root (Polkit/sudo) dimunculkan, ArchDroid **selalu menampilkan modal konfirmasi**:
- Menjelaskan perintah persis yang akan dieksekusi.
- Merinci paket sistem distro yang akan dipasang.
- Backend Rust memvalidasi kembali bahwa paket yang dieksekusi sama persis dengan yang disetujui pengguna (menolak injeksi paket liar).
- Script root dieksekusi di direktori privat sementara (`mode 0700`) dan mengisolasi cache agar tidak mencemari direktori pengguna.

---

## 6. Pemecahan Masalah (Troubleshooting)

### 1. Dialog Root Tidak Muncul di Desktop Mandiri (Hyprland / Sway / i3)
- **Gejala**: Tombol install dependensi atau init hang/timeout tanpa ada popup password.
- **Penyebab**: Window manager mandiri tidak menyertakan Polkit Authentication Agent secara default.
- **Solusi**: Pasang dan jalankan agen polkit di sesi startup Anda:
  ```bash
  # Untuk Hyprland
  systemctl --user enable --now hyprpolkitagent.service
  # Atau jalankan polkit-gnome / polkit-kde di autostart
  /usr/lib/polkit-gnome/polkit-gnome-authentication-agent-1 &
  ```
  Atau jalankan `archdroid` melalui terminal untuk melihat prompt password `sudo` berbasis teks.

### 2. Modul Kernel Binder Hilang (`binder: missing`)
- **Gejala**: Diagnostik menunjukkan Binder hilang; Waydroid tidak bisa start container.
- **Penyebab**: Kernel Linux yang digunakan tidak mengaktifkan modul `binder_linux` atau binderfs.
- **Solusi**:
  - Pada Arch Linux: Gunakan kernel standar `linux` atau `linux-zen` yang sudah mengaktifkan binder secara bawaan.
  - Jika menggunakan kernel custom, pastikan opsi berikut aktif:
    ```ini
    CONFIG_ANDROID_BINDER_IPC=y
    CONFIG_ANDROID_BINDERFS=y
    ```
  - Pastikan binderfs di-mount:
    ```bash
    sudo mkdir -p /dev/binderfs
    sudo mount -t binder binder /dev/binderfs
    ```

### 3. Wayland Compositor Tidak Terdeteksi (`Wayland: not running`)
- **Gejala**: Aplikasi memperingatkan bahwa sesi Wayland tidak ditemukan.
- **Penyebab**: Anda sedang login pada sesi desktop X11 murni (`DISPLAY` aktif, tetapi `WAYLAND_DISPLAY` kosong).
- **Solusi**: Login ke sesi Wayland (pilih "GNOME on Wayland", "Plasma (Wayland)", atau jalankan compositor seperti Hyprland/Sway). Untuk menjalankan di sesi X11, Anda dapat menggunakan nested Wayland compositor seperti `weston` atau `cage`.

### 4. Izin KVM Ditolak (`/dev/kvm: permission denied`)
- **Gejala**: Performa emulasi lambat atau peringatan izin KVM.
- **Solusi**: Tambahkan pengguna Anda ke grup `kvm`:
  ```bash
  sudo usermod -aG kvm $USER
  ```
  Lalu log out dan log in kembali.

### 5. Kompatibilitas GPU (Nvidia vs AMD / Intel)
- **Intel & AMD**: Bekerja langsung secara out-of-the-box menggunakan driver open-source Mesa (DRI3/Vulkan).
- **Nvidia Proprietary**: Pastikan modul `nvidia-drm.modeset=1` aktif pada boot parameter kernel. Jika Waydroid mengalami render glitch, Waydroid dapat dikonfigurasi untuk menggunakan fallback software renderer (`Mesa llvmpipe`).

### 6. Melaporkan Masalah (Diagnostic Bundle)
Buka **Settings → Diagnostic Bundle → Create Bundle**. ArchDroid menulis satu file teks ke `~/Downloads/archdroid-diagnostics-<waktu>.txt` berisi versi sistem dan Waydroid, hasil diagnostik host, status add-on, serta log terakhir, dengan path home dan nilai token/password disamarkan. Log dapat memuat nama paket Android dan nama file APK yang pernah Anda pakai; baca isinya dan hapus yang tidak ingin dibagikan, lalu lampirkan ke laporan bug. File ini dibuat tanpa root dan tidak pernah diunggah otomatis.

---

## 7. Panduan Uninstall Bersih (Clean Uninstall)

Jika Anda ingin menghapus ArchDroid dan seluruh datanya dari sistem:

### 1. Menghapus Aplikasi ArchDroid
- **Arch Linux**:
  ```bash
  sudo pacman -R archdroid-bin
  # Atau jika dipasang dari source PKGBUILD:
  sudo pacman -R archdroid
  ```
- **Debian / Ubuntu**:
  ```bash
  sudo apt remove archdroid
  ```
- **Fedora / openSUSE**:
  ```bash
  sudo dnf remove archdroid
  ```
- **AppImage**: Cukup hapus file `ArchDroid_*.AppImage` dan file `.desktop` terkait di `~/.local/share/applications/` jika ada.

### 2. Menghapus Konfigurasi & Cache Pengguna
Hapus preferensi lokal, keymap Phantom, dan cache sementara:
```bash
rm -rf ~/.config/archdroid
rm -rf ~/.cache/archdroid
```

### 3. Menghapus Data & Image Waydroid (Opsional)
Jika Anda juga ingin menghapus seluruh sistem Android, aplikasi yang terpasang di dalam container, dan image Waydroid:
```bash
# Hentikan sesi dan service Waydroid
waydroid session stop
sudo systemctl stop waydroid-container.service

# Hapus data sistem dan image container (membutuhkan root)
sudo rm -rf /var/lib/waydroid

# Hapus data runtime pengguna
rm -rf ~/.local/share/waydroid
```

---

## 8. Batasan yang Diketahui (Known Limitations)

1. **Satu Container per Host (Single Instance Container)**:
   Waydroid secara arsitektur menggunakan satu container LXC global per mesin host. ArchDroid tidak mendukung multi-instance container Android simultan secara paralel.
2. **Ketergantungan Sesi Wayland**:
   ArchDroid dan Waydroid membutuhkan Wayland compositor aktif. Tidak ada display server terisolasi X11 bawaan di dalam aplikasi.
3. **Ketersediaan ARM Translation Layer (libhoudini)**:
   Injeksi libhoudini bergantung pada ketersediaan image Google Android TV upstream yang diunduh melalui skrip helper ter-pin. Game yang menggunakan sistem anti-cheat tingkat kernel (seperti game tertentu dengan proteksi ketat) mungkin tidak dapat berjalan pada container emulator.
4. **Tingkat Dukungan Distro**:
   Dukungan penuh (Tier 1) yang diverifikasi live mencakup keluarga Arch Linux. Distro Tier 2 (Debian, Ubuntu, Fedora, openSUSE) berstatus preview dan belum melalui pengujian live di seluruh varian rilisnya.

---

## 9. Fitur Utama

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

## 10. Keamanan & Privilege Boundary

ArchDroid menerapkan prinsip keamanan defensif berlapis:
- **Hash-Pinned Python Dependencies**: Dependensi Python untuk helper instalasi di-lock dan diverifikasi dengan hash SHA-256 (`--require-hashes --only-binary=:all:`).
- **Isolasi Lingkungan Root**: Script privileged berjalan di direktori temporer privat (`mktemp -d`, mode `0700`), `SUDO_USER` di-unset, dan cache diarahkan ke `$ARCHDROID_WORK/cache` agar proses root tidak pernah menulis file ke `~/.cache` milik user desktop.
- **Explicit Consent**: Semua installer root wajib dikonfirmasi melalui modal persetujuan pengguna sebelum prompt root polkit/sudo muncul.
- **Package Approval**: Daftar paket sistem dan command persis yang akan dijalankan ditampilkan di modal; backend memvalidasi ulang daftar yang disetujui dan menolak instalasi paket di luar rencana (`approval_required` / `invalid_input`).
- **Fail-Fast Preflight**: Jika dependensi hilang pada distribusi yang tidak didukung, sistem menolak operasi sebelum meminta izin root.

---

## 11. Pengembangan & Build

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

## 12. Lisensi & Upstream Credits

- **ArchDroid**: Lisensi [MIT](LICENSE), Copyright (c) 2026 wisangdg.
- **Waydroid**: [https://waydroid.net](https://waydroid.net) (GPL-3.0).
- **waydroid_script**: [casualsnek/waydroid_script](https://github.com/casualsnek/waydroid_script) (GPL-3.0) — Pinned commit `48dbfaf`. Script ini diunduh saat instalasi add-on dan tidak di-vendor di dalam binary ArchDroid.
- **Phantom Keymapper**: [oliviermugishak/phantom](https://github.com/oliviermugishak/phantom) (MIT) — Pinned commit `1a6b7ed`.
- Rincian pihak ketiga dan audit lisensi tersedia di [`docs/THIRD-PARTY.md`](docs/THIRD-PARTY.md).

> [!NOTE]
> **Pernyataan Merek Dagang (Trademark Disclaimer)**: LDPlayer dan NoxPlayer adalah merek dagang atau merek dagang terdaftar milik pemiliknya masing-masing. Penggunaan nama tersebut di dalam dokumentasi semata-mata bertujuan sebagai referensi perbandingan antarmuka pengguna. ArchDroid adalah proyek independen dan tidak berafiliasi, disponsori, atau didukung oleh pemegang merek dagang tersebut.
