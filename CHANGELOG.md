# Changelog

Semua perubahan penting pada proyek ArchDroid akan dicatat dalam dokumen ini.
Format ini mengacu pada [Keep a Changelog](https://keepachangelog.com/id/1.0.0/) dan mengikuti [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.1.0] - 2026-10-31

### Ditambahkan
- **App Library & Game Launcher**:
  - Manajemen visual aplikasi Android yang terpasang di container Waydroid.
  - Ekstraksi ikon aplikasi otomatis, pencarian instan (`Ctrl+F`), dan filter kategori (All/User/System).
  - Menu konteks klik kanan (Launch, Stop, Uninstall, App Info).
- **Drag-and-Drop APK Installer**:
  - Dukungan pemasangan APK secara langsung dengan menarik file `.apk` ke dalam jendela aplikasi.
- **Inisialisasi Container & Setup Wizard**:
  - Deteksi otomatis status Waydroid dan image Android.
  - Wizard inisialisasi interaktif dengan pemilihan image `VANILLA` murni vs `GAPPS` (Google Play Services).
  - Host preflight diagnostics visual sebelum inisialisasi dijalankan.
- **Injeksi Add-on Sistem (GApps & ARM Translation)**:
  - Injeksi Google Mobile Services (MindTheGapps / OpenGApps).
  - Injeksi Intel libhoudini ARM translation layer untuk menjalankan game Android 3D mobile pada CPU x86_64.
  - Injeksi deklarasi hardware multitouch untuk kompatibilitas Google Play Store.
- **Keamanan & Privilege Boundaries (Defensive Security)**:
  - Dialog persetujuan eksplisit (*explicit consent*) dan peninjauan daftar paket distro (*package approval gating*) sebelum eskalasi hak akses root.
  - Penolakan eksekusi otomatis jika ada paket distro di luar daftar yang disetujui.
  - Isolasi eksekusi skrip root dalam direktori privat `mode 0700` (`mktemp -d`).
  - Pembersihan environment sensitif (`SUDO_USER`) dan pengalihan cache root ke `$ARCHDROID_WORK/cache` agar tidak mencemari direktori pengguna.
  - Pengecekan hash SHA-256 pada seluruh dependensi Python installer (`--require-hashes`).
  - Content Security Policy (CSP) produksi yang ketat (`default-src 'self'`) tanpa remote font, CDN, atau inline string injection.
  - Scoping capability IPC Tauri v2 yang memblokir akses plugin arbitrary shell/fs/http.
- **Display Profiles & Custom DPI**:
  - Pilihan preset resolusi cepat (Tablet 1080p, Tablet 720p, Mobile Portrait, Gaming 2K).
  - Kustomisasi DPI dinamis dengan verifikasi pembacaan ulang properti Android.
- **Smart Keymapping (Phantom Integration)**:
  - Integrasi dengan keymapper virtual touchscreen Phantom di level pengguna *unprivileged*.
- **Diagnostik Sistem Host (Host Diagnostics)**:
  - 17 pemeriksaan kesehatan sistem secara langsung (KVM, binder, `/dev/dri`, Wayland, Polkit, disk, memori, jaringan).
- **Multi-Distro Packaging & Verification**:
  - Target resmi Tier 1 untuk Arch Linux: `packaging/arch/PKGBUILD` dan binary package `archdroid-bin-*.pkg.tar.zst`.
  - Standalone executable AppImage dengan pembersihan environment `PYTHONHOME`/`PYTHONPATH` bawaan AppRun.
  - Target eksperimental Tier 2 (preview `.deb` dan `.rpm`).
  - Penandatanganan rilis resmi dengan kunci GPG Ed25519 (`18F47067ACC5BBF9`) serta skrip verifikasi otomatis `verify-release.sh`.
- **Dokumentasi & Standar Komunitas**:
  - `README.md` lengkap dengan panduan instalasi, model privilege root, troubleshooting, dan clean uninstall.
  - `SECURITY.md` dengan alur pelaporan celah keamanan privat.
  - `PRIVACY.md` yang menjamin zero telemetry, zero analytics, dan zero phone-home.
  - `NOTICES.md` mencatat lisensi dan atribusi pihak ketiga.
