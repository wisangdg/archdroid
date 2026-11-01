# Changelog

Semua perubahan penting pada proyek ArchDroid akan dicatat dalam dokumen ini.
Format ini mengacu pada [Keep a Changelog](https://keepachangelog.com/id/1.0.0/) dan mengikuti [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Diperbaiki
- Panduan uninstall memakai nama paket yang benar untuk `.deb`/`.rpm` (`arch-droid`, bukan `archdroid`), dan contoh instalasi memakai nama file yang sebenarnya (`ArchDroid_*.deb`, `ArchDroid-*.rpm`); sebelumnya perintah itu gagal.
- Start Engine kini melaporkan "Start Failed" beserta penyebab dari Waydroid (mis. `OSError: container failed to start`) bila container gagal start. Sebelumnya `waydroid session start` keluar dengan kode 0 setelah ~10 detik dan ArchDroid hanya menampilkan "Session Slow to Start". Start yang berhasil dilaporkan begitu status menunjukkan RUNNING.

## [0.1.3] - 2026-11-01

Rilis perbaikan setelah uji live Tier 2 di Ubuntu 24.04. Paket `.deb` kini ikut dilampirkan dan ditandatangani.

### Diperbaiki
- Paket `.deb` (Tier 2 preview) tidak bisa dipasang di Ubuntu 24.04 karena mewajibkan paket `waydroid` yang tidak ada di repo resmi Ubuntu. `waydroid` kini menjadi *Recommends*; layar setup ArchDroid sudah menangani host tanpa Waydroid.
- Di host `apt`, item diagnostik "Waydroid Package" kini menyebut repo resmi Waydroid bila `apt` tidak menemukan paket `waydroid` (mis. Ubuntu 24.04).
- Daftar app kini diperbarui sendiri selama sesi berjalan. Sebelumnya, pada boot pertama setelah init (saat Waydroid baru menulis entri launcher) daftar tetap "0 applications" sampai ada aksi lain.
- Peringatan "Session Slow to Start" kini mengarahkan ke Diagnostic Bundle, yang memuat log Waydroid. `waydroid session start` dapat keluar dengan kode 0 walaupun container gagal start.

## [0.1.2] - 2026-11-01

Rilis perbaikan setelah uji live CSP runtime dan drag-and-drop native di VM.

### Ditambahkan
- `archdroid --version` (atau `-V`) mencetak versi tanpa membuka jendela, sesuai panduan upgrade di `docs/RELEASE-AND-UPDATES.md`.
- Diagnostic bundle (Settings → Diagnostic Bundle): satu file teks untuk laporan bug berisi versi OS/kernel/Waydroid, diagnostik host, status add-on, dan log terakhir. Dikumpulkan tanpa root, disamarkan, disimpan dengan mode `0600` di `~/Downloads`, dan tidak pernah diunggah.

### Diperbaiki
- Baris log yang panjang di Settings (Application Log, Diagnostic Bundle) kini terbungkus dan tidak lagi melebarkan kartu.
- `PRIVACY.md` menyebut lokasi preferensi dan log aplikasi yang sebenarnya.
- `PRIVACY.md` kini mengungkap probe konektivitas otomatis (koneksi TCP tanpa data ke `1.1.1.1:53`, lalu `8.8.8.8:53` bila gagal) yang sudah ada sejak 0.1.0. Sebelumnya dokumen menyatakan semua koneksi jaringan hanya terjadi atas permintaan pengguna.
- Teks panel Diagnostic Bundle, README, dan `PRIVACY.md` menyebut bahwa log di dalam bundle dapat memuat nama paket Android dan nama file APK.
- Setelah Cancel, stepper tetap menandai tahap yang sudah selesai dan berhenti berputar, bukan kembali ke tahap 1.
- Judul terminal wipe tidak lagi bertuliskan "in Progress..." setelah selesai.
- Favicon WebView memakai ikon ArchDroid; sebelumnya merujuk `/vite.svg` yang tidak ada.
- Install/uninstall APK saat container Waydroid sedang beku (*frozen*) kini langsung memperbarui daftar app. Sebelumnya container dibekukan lagi sesaat setelah `adb install`, sebelum Waydroid menulis entri launcher, sehingga app yang berhasil terpasang tidak muncul sampai container aktif kembali.
- "Launch Keymapper" kini melaporkan error bila `phantom-gui` (atau terminal fallback) langsung keluar setelah dijalankan, bukan menampilkan "Keymapper Launched". Proses yang tetap berjalan di-*reap* agar tidak menjadi zombie.
- Pesan setelah Cancel tidak lagi menyatakan "No modifications were completed"; tahap yang sudah selesai tetap diterapkan, dan tahap yang terputus mungkin belum lengkap.

## [0.1.1] - 2026-11-01

Rilis pertama yang dipublikasikan. Artefak 0.1.0 tidak pernah dirilis: smoke test live di VM menemukan bug pembersihan pada Cancel/timeout task root.

### Diperbaiki
- Cancel atau timeout pada task root (GApps, Houdini, multitouch, init, wipe) kini benar-benar menghapus direktori kerja privat `/var/tmp/archdroid.*`. Sebelumnya `set -e` membuat trap berhenti sebelum `rm -rf`, sehingga unduhan dan venv milik root tertinggal.
- Task root yang dibatalkan kini berhenti di tempat; skrip tidak lagi melanjutkan ke perintah root berikutnya setelah sinyal.
- Progress bar installer (tqdm/pip) tidak lagi tampil rusak (`â▯▯`): output kini didekode sebagai UTF-8 per baris, bukan per byte.
- Modal terminal tidak lagi meluap dari jendela 1100×720, sehingga tombol Cancel di header dan tombol di footer selalu terlihat.

## [0.1.0] - 2026-11-01

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
- **Keandalan Operasi**:
  - Setiap task panjang punya `task_id`, deadline, dan tombol Cancel (termasuk install/remove APK); task kedua ditolak dengan code `busy`.
  - Install dan uninstall APK diverifikasi lewat `adb` (fallback `waydroid app`), sehingga "sukses" berarti paket benar-benar terpasang atau terhapus.
  - Perubahan properti dan resolusi dibaca ulang dari Android; toast sukses hanya muncul jika nilainya benar-benar berubah.
  - Profil perangkat diberi versi dan disimpan secara atomik, dengan migrasi untuk file lama.
- **First Run & Log**:
  - Layar setup bertahap (Waydroid belum terpasang, belum diinisialisasi, sedang inisialisasi, siap).
  - Panel log aplikasi dengan lokasi file log dan tombol hapus; isi skrip root tidak ditulis ke log dan pesan error diredaksi.
- **Aksesibilitas**:
  - Navigasi keyboard penuh, focus trap pada modal, dan semantik ARIA untuk kartu aplikasi, menu, dan dialog.
- **Dokumentasi & Standar Komunitas**:
  - `README.md` lengkap dengan panduan instalasi, model privilege root, troubleshooting, dan clean uninstall.
  - `SECURITY.md` dengan alur pelaporan celah keamanan privat.
  - `PRIVACY.md` yang menjamin zero telemetry, zero analytics, dan zero phone-home.
  - `NOTICES.md` mencatat lisensi dan atribusi pihak ketiga.
