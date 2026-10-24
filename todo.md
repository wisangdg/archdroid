# ArchDroid — TODO Production Readiness

> **Status: BELUM production-ready.**
> Basis pemeriksaan: commit `639b5ec` (24 Okt 2026), ditambah perbaikan
> P0-3.1, P0-3.2, dan P0-3.3.
> P0-1/P0-2 (hardening boundary privilege dan pin installer), P0-3.1
> (hasil operasi privileged ditunggu sampai proses selesai), P0-3.2
> (hasil terstruktur), dan P0-3.3 (single-flight dan `task_id`) sudah selesai;
> P0-4 (timeout/cancel) masih terbuka.
> Dokumen ini adalah **gate rilis**, bukan roadmap fitur. Fitur di
> [`docs/BACKEND-TODO.md`](docs/BACKEND-TODO.md) tidak otomatis menjadi blocker
> kecuali memutus alur inti yang sudah ada.

## 1. Ringkasan status

### Sudah selesai / sudah terverifikasi

- [x] Hardening boundary privilege backend: resolusi user/home dari sistem,
  `PATH` tetap untuk root, environment root disanitasi, workdir privat.
- [x] Installer pihak ketiga di-pin ke commit immutable; Phantom berjalan
  sebagai user. Lihat [`docs/THIRD-PARTY.md`](docs/THIRD-PARTY.md).
- [x] Error backend bertipe dan punya `code` stabil di `CommandResult`.
- [x] Command Waydroid yang berpotensi lama sudah non-blocking; install/remove
  APK bisa dibatalkan.
- [x] P0-3.1: command privileged menunggu proses selesai dan mengembalikan
  hasil final; marker instalasi hanya ditulis setelah exit 0.
- [x] P0-3.2: `cmd-finished` mengirim objek
  `{ task_id, success, code, message }`; UI memakai `code` untuk membedakan
  error root, timeout, dan cancel tanpa mencocokkan teks pesan.
- [x] P0-3.3: single-flight backend dan frontend. `TaskGuard` memberi
  `task_id` stabil, menolak task kedua dengan code `busy`, dan melepas slot
  pada sukses, gagal, panic, atau future pemanggil yang dibatalkan.
  `cmd-output` mengirim `{ task_id, line }`; `cmd-finished` mengirim
  `task_id` nyata.
- [x] `cargo fmt --check` lulus.
- [x] `cargo test` lulus: **86 test**.
- [x] `cargo clippy --all-targets -- -D warnings` lulus.
- [x] `npm run build` lulus.
- [x] `.deb` berhasil dibangun:
  `src-tauri/target/release/bundle/deb/ArchDroid_0.1.0_amd64.deb`.
- [x] `npm audit --omit=dev`: **0 vulnerability** pada dependency runtime.

### Belum selesai / belum diverifikasi

- [ ] Uji live Waydroid, polkit/sudo, installer root, dan wipe di mesin/VM
  bersih. Waydroid tidak tersedia/berjalan di environment pengembangan ini.
- [ ] CSP produksi masih `null`; font masih diambil dari Google Fonts.
- [ ] Uji live pkexec/sudo untuk hasil P0-3.1 (sukses, batal, exit non-zero,
  sudo gagal) tetap harus dilakukan di P0-8.
- [ ] Timeout/cancel belum berlaku untuk script privileged; Cancel baru
  menghentikan `waydroid app install/remove` (P0-4.1/P0-4.2).
- [ ] Dependency Python `waydroid_script` belum di-hash-pin.
- [ ] Dukungan distro masih ambigu: script memakai `pacman`, tetapi artefak
  yang dibangun adalah `.deb`.
- [ ] CI belum pernah berjalan di remote/PR.
- [ ] Belum ada test/lint frontend dan belum ada E2E.
- [ ] Hanya `.deb` yang dibangun; target bundle lain belum, dan instalasi
  `.deb` belum diuji di VM bersih.
- [ ] README, LICENSE, SECURITY, PRIVACY, dan CHANGELOG belum ada.
- [ ] `npm audit` penuh masih melaporkan **7 vulnerability dev-only**
  (4 high), terutama dari rantai Vite 5.4.21.

**Kesimpulan:** build dan test backend sudah sehat, tetapi aplikasi belum layak
disebut production-ready sampai seluruh **P0** di bawah selesai dan diverifikasi
secara live.

---

## 2. P0 — Release blockers

Urutan ini sengaja dibuat dari yang paling berisiko: keamanan eksekusi root,
isolasi UI, lalu validasi rilis.

### P0-3 — Propagasi hasil operasi privileged

- [x] **P0-3.1 — Jangan kembalikan `success` sebelum proses selesai.**
  - **Bukti:** `stream_privileged_script()` di `src-tauri/src/lib.rs` berjalan
    di thread terpisah, sementara `waydroid_init`, `waydroid_wipe`,
    `waydroid_reboot`, `install_gapps`, `install_houdini`,
    `install_multitouch`, dan `install_phantom` langsung mengembalikan
    `CommandResult::success()`.
  - **Dampak:** polkit dibatalkan, script gagal, atau sudo tidak tersedia tetap
    terlihat sukses oleh pemanggil. `handleInstallMultitouch()` bahkan langsung
    menampilkan toast “Restart Required” sebelum hasil root diketahui.
  - **Kerjakan:** buat `stream_privileged_script` mengembalikan hasil akhir
    (misalnya `oneshot`/future), lalu command `await` hasil itu dan baru
    memetakan ke `CommandResult`. Event `cmd-output` tetap dipakai untuk
    progres. Jangan menulis marker instalasi sebelum exit sukses.
  - **DoD:** tes dengan pkexec sukses, pkexec dibatalkan, exit non-zero, dan
    sudo gagal; semua menghasilkan status/`code` yang benar. UI tidak pernah
    menampilkan sukses sebelum `cmd-finished`/hasil final.
  - **Status:** implementasi selesai. Unit test mapping hasil lulus; eksekusi
    pkexec/sudo nyata tetap diverifikasi di P0-8 karena environment ini tidak
    punya Waydroid/polkit.

- [x] **P0-3.2 — Kirim hasil terstruktur, bukan hanya boolean.**
  - **Bukti:** `cmd-finished` saat ini hanya mengirim `true/false`, sehingga
    kode error (`elevation_denied`, `timeout`, `cancelled`, `exit`) hilang.
  - **Kerjakan:** payload event minimal
    `{ task_id, success, code, message }`; UI memakai `code` untuk pesan dan
    tindakan lanjutan.
  - **DoD:** error root, timeout, dan cancel bisa dibedakan di UI tanpa
    mencocokkan teks pesan.
  - **Status:** implementasi selesai. Unit test memverifikasi payload
    `elevation_denied`, `timeout`, dan `cancelled`; produksi timeout/cancel
    nyata tetap dikerjakan di P0-4. Sejak P0-3.3 `task_id` berisi ID nyata.

- [x] **P0-3.3 — Single-flight dan `task_id` untuk operasi privileged.**
  - **Bukti:** event `cmd-output`/`cmd-finished` global; belum ada pengunci
    backend yang mencegah dua installer root berjalan bersamaan.
  - **Kerjakan:** tolak operasi kedua saat ada task aktif, atau beri `task_id`
    dan routing event per task. Bersihkan state saat task selesai/gagal.
  - **DoD:** dua klik cepat tidak mencampur log, tidak memunculkan dua prompt
    root, dan Cancel hanya menghentikan task yang dimaksud.
  - **Status:** implementasi selesai. `TaskGuard` global memberi ID `task-N`,
    menolak task kedua dengan code `busy` tanpa mengirim event, dan melepas
    slot pada sukses, gagal, panic, atau future pemanggil yang dibatalkan.
    Slot dipegang di thread script, jadi script yang sudah mulai tetap
    single-flight walau future Tauri dijatuhkan. `cmd-output` mengirim
    `{ task_id, line }`; `cmd-finished` mengirim `task_id` nyata. Frontend
    memakai ref single-flight dan menangani `busy` tanpa mereset terminal.
    Cancel per task (GApps/Houdini/init/wipe/Phantom/log) tetap P0-4.2; P0-3.3
    hanya menjamin tidak ada task lain yang bisa menerima Cancel.

### P0-4 — Timeout, cancel, dan pembersihan script privileged

- [ ] **P0-4.1 — Tambahkan deadline untuk semua script privileged/user.**
  - **Bukti:** `run_and_stream()` menunggu child tanpa deadline;
    `stream_privileged_script()` tidak punya timeout. `cancel` yang ada hanya
    flag `waydroid::CANCEL` untuk `app install/remove`, bukan installer.
  - **Dampak:** `pacman`, `git`, `pip`, `waydroid init`, atau installer yang
    menggantung membuat UI terus `installing` dan tidak bisa ditutup dengan
    benar.
  - **Kerjakan:** deadline per jenis task (init/installer boleh panjang,
    operasi lain lebih pendek), pembunuhan **process group** agar anak pkexec/
    sudo/bash ikut mati, lalu cleanup workdir.
  - **DoD:** proses `sleep` yang disisipkan sebagai stub bisa dibunuh; timeout
    menghasilkan `timeout`; tidak ada proses yatim setelah Cancel.

- [ ] **P0-4.2 — Hubungkan tombol Cancel ke task yang benar.**
  - **Bukti:** `TerminalModal` menampilkan Cancel untuk semua task, tetapi
    `handleCancelTask()` memanggil `waydroid_cancel`, yang hanya memengaruhi
    `waydroid app install/remove`.
  - **Kerjakan:** endpoint cancel per task atau `cancel_task(task_id)`, termasuk
    installer GApps/Houdini, init, wipe, Phantom, dan log.
  - **DoD:** Cancel menghentikan proses dalam hitungan detik, mengirim
    `cancelled`, menutup prompt bila memungkinkan, dan UI kembali ke state
    idle tanpa menunggu timeout.

- [ ] **P0-4.3 — Uji failure path, bukan hanya happy path.**
  - **DoD:** ada test/stub untuk exit 1, polkit 126, sudo gagal, hang, cancel,
    dan cleanup workdir; tidak ada marker “installed” palsu.

### P0-5 — CSP produksi dan aset offline

- [ ] **P0-5.1 — Isi `app.security.csp`; jangan biarkan `null`.**
  - **Bukti:** `src-tauri/tauri.conf.json` masih `"csp": null`.
  - **Kerjakan:** CSP produksi minimal: `default-src 'self'`,
    `script-src 'self'`, `style-src 'self' 'unsafe-inline'` bila memang
    dibutuhkan React inline style, `img-src 'self' data:`,
    `font-src 'self'`, `object-src 'none'`, `frame-src 'none'`, dan
    `connect-src` hanya untuk IPC Tauri yang dipakai. Tambahkan `devCsp` untuk
    Vite/WebSocket, jangan melemahkan CSP produksi.
  - **DoD:** tidak ada pelanggaran CSP di DevTools; aplikasi tetap berfungsi
    saat offline; tidak ada script/gaya dari host remote.

- [ ] **P0-5.2 — Hapus Google Fonts remote atau self-host.**
  - **Bukti:** baris pertama `src/index.css` memuat
    `https://fonts.googleapis.com/...`.
  - **Dampak:** aplikasi bergantung pada jaringan, membocorkan request ke
    pihak ketiga, dan tidak kompatibel dengan CSP ketat.
  - **Kerjakan:** gunakan font sistem, atau bundle font (mis. Outfit dan
    JetBrains Mono) ke `src/assets/fonts/` dengan lisensi yang sesuai.
  - **DoD:** aplikasi terlihat benar tanpa internet; tidak ada request ke
    Google; lisensi font dicatat.

- [ ] **P0-5.3 — Audit permukaan webview.**
  - **Kerjakan:** pastikan tidak ada `dangerouslySetInnerHTML`/HTML dari input
    pengguna; tinjau `src-tauri/capabilities/default.json`; jangan menambah
    plugin shell/FS tanpa kebutuhan.
  - **DoD:** capability minimum, CSP aktif, dan daftar permission ditinjau di
    PR rilis.

### P0-6 — Supply chain installer yang berjalan sebagai root

- [ ] **P0-6.1 — Hash-pin dependency Python `waydroid_script`.**
  - **Bukti:** `gapps_install_script()` dan `houdini_install_script()` membuat
    venv lalu menjalankan
    `pip install -r requirements.txt` sebagai root tanpa hash. Commit repo
    sudah di-pin, tetapi isi PyPI masih bisa berubah.
  - **Kerjakan:** buat lockfile dengan versi + hash, jalankan
    `pip install --require-hashes --only-binary=:all:` bila memungkinkan,
    dan verifikasi lockfile sebelum eksekusi. Jika upstream tidak menyediakan
    lock stabil, vendor dependency yang sudah diaudit atau turunkan installer
    ini dari build produksi dan arahkan pengguna ke upstream.
  - **DoD:** instalasi GApps/Houdini dapat direproduksi; tidak ada dependency
    yang berubah tanpa review; `docs/THIRD-PARTY.md` diperbarui.

- [ ] **P0-6.2 — Audit ulang installer upstream dan izinnya.**
  - **Kerjakan:** tinjau `install.sh`/`main.py` pada commit yang di-pin,
    pastikan hanya menulis ke path yang diharapkan, tidak mengambil script
    tambahan dari URL yang tidak terverifikasi, dan tidak membaca environment
    user yang sensitif.
  - **DoD:** daftar file/path yang disentuh terdokumentasi; tidak ada eksekusi
    kode arbitrer di luar source yang di-pin.

- [ ] **P0-6.3 — Jangan biarkan installer root menjadi jalur default tanpa
  persetujuan eksplisit.**
  - **Kerjakan:** tampilkan peringatan bahwa installer mengubah image Waydroid
    dan meminta root; jelaskan sumber, versi, dan konsekuensinya.
  - **DoD:** pengguna melihat konfirmasi sebelum prompt polkit/sudo.

### P0-7 — Kontrak platform dan preflight dependency

- [ ] **P0-7.1 — Putuskan target distro secara eksplisit.**
  - **Bukti:** `waydroid_init`, GApps, dan Houdini memanggil `pacman`, tetapi
    artefak yang dibangun adalah `.deb`. Ini kontrak produk yang saling
    bertentangan.
  - **Pilihan:**
    - **Arch-only:** dokumentasikan Arch Linux + Waydroid + Wayland sebagai
      target resmi; sediakan paket Arch/AUR; jangan publikasikan `.deb` sebagai
      didukung.
    - **Multi-distro:** tambahkan `apt-get`/`dnf`/`zypper`, pemetaan paket,
      dan matriks uji; jangan asumsikan nama paket sama.
  - **DoD:** README menyebut target yang didukung dan tidak didukung; installer
    gagal cepat dengan pesan jelas pada distro yang tidak didukung, sebelum
    meminta root.

- [ ] **P0-7.2 — Preflight dependency sebelum aksi.**
  - **Kerjakan:** deteksi `waydroid`, `adb` (android-tools), Wayland, `/dev/kvm`,
    binder, `/dev/dri`, polkit agent atau sudo, `git`, `python3` +
    `python3-venv`, `squashfs-tools`, `lzip`, `xdg-open`, terminal emulator
    untuk Phantom, ruang disk, dan koneksi jaringan sesuai aksi.
  - **DoD:** setup wizard/diagnostik menampilkan item yang kurang beserta
    perintah perbaikannya; tidak ada installer yang baru gagal setelah prompt
    root.

- [ ] **P0-7.3 — Jangan instal paket sistem secara diam-diam.**
  - **Bukti:** `waydroid_init` menjalankan `pacman -S --noconfirm waydroid`
    bila `waydroid` tidak ada.
  - **Kerjakan:** tampilkan perintah yang akan dijalankan dan minta
    persetujuan; gunakan `--needed`; jangan lakukan upgrade sistem penuh.
  - **DoD:** pengguna tahu perubahan paket yang dilakukan.

### P0-8 — Validasi live end-to-end

- [ ] **P0-8.1 — Siapkan VM/mesin uji sekali pakai.**
  - **DoD:** ada snapshot bersih dengan Waydroid, polkit, dan data uji; tidak
    memakai akun/data pribadi.

- [ ] **P0-8.2 — Jalankan matriks alur utama.**
  - Fresh host tanpa Waydroid.
  - Waydroid terpasang tetapi belum `init`.
  - Init, start, stop, restart, status, dan kondisi container `FROZEN`.
  - Install/launch/uninstall APK; drag-and-drop APK; APK invalid.
  - GApps, Houdini, multitouch; verifikasi marker dan status live.
  - Phantom install/run; terminal tidak tersedia.
  - Wipe di snapshot disposable; pastikan path dan marker benar.
  - Polkit tersedia, polkit dibatalkan, tanpa polkit agent, fallback sudo.
  - Tanpa `adb`, tanpa network, disk penuh, dan X11.
  - Cancel dan timeout pada setiap operasi panjang.
  - **DoD:** hasil dicatat dalam tabel uji (langkah, hasil, log); semua kegagalan
    punya pesan yang dapat ditindaklanjuti; tidak ada data pengguna nyata.

- [ ] **P0-8.3 — Verifikasi tidak ada regresi data.**
  - **DoD:** wipe tidak menyentuh path di luar Waydroid/home yang divalidasi;
    marker dihapus/ditulis hanya pada hasil yang sesuai.

### P0-9 — Packaging dan artefak rilis

- [ ] **P0-9.1 — Bangun dan uji target yang benar-benar didukung.**
  - **Bukti:** konfigurasi `targets: "all"`, tetapi hanya `.deb` yang pernah
    dibangun; belum ada uji install untuk artefak mana pun.
  - **Kerjakan:** putuskan deb/AppImage/rpm/AUR; bangun semuanya; uji install,
    launch dari menu aplikasi, dan uninstall di VM bersih.
  - **DoD:** setiap artefak yang dipublikasikan berhasil dipasang dan dijalankan
    tanpa terminal; tidak ada dependency runtime yang hilang.

- [ ] **P0-9.2 — Lengkapi metadata paket.**
  - **Kerjakan:** `bundle` metadata: homepage, repository, license, copyright,
    publisher, dan dependency runtime yang sesuai. Pastikan desktop entry,
    ikon, dan nama binary benar.
  - **DoD:** metadata paket dapat diperiksa dengan `dpkg-deb`/equivalen dan
    tidak ada klaim distro yang menyesatkan.

- [ ] **P0-9.3 — Tanda tangani/verifikasi artefak.**
  - **Kerjakan:** publikasikan `SHA256SUMS` dan signature GPG minimal; jika
    memakai auto-update, gunakan signature Tauri updater dan key terpisah.
  - **DoD:** pengguna dapat memverifikasi artefak sebelum memasang.

### P0-10 — CI dan gate rilis di remote

- [ ] **P0-10.1 — Jalankan CI di remote/PR, bukan hanya lokal.**
  - **Bukti:** `.github/workflows/ci.yml` ada, tetapi repo belum punya remote
    dan workflow belum pernah hijau.
  - **DoD:** PR dan `main` menjalankan workflow yang required; hasilnya hijau.

- [ ] **P0-10.2 — Tambahkan pemeriksaan backend/build yang hilang.**
  - **Kerjakan:** frontend build (`npm ci` + `npm run build`), `cargo fmt
    --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`,
    bundle build, `cargo audit`/`cargo deny`, `npm audit --omit=dev`, dan
    pengecekan lockfile. Pemeriksaan lint/test frontend ditambahkan bersama
    P1-2.
  - **DoD:** dependency rentan dan bundle yang gagal build memblokir PR rilis.

- [ ] **P0-10.3 — Pin GitHub Actions ke commit SHA.**
  - **DoD:** action pihak ketiga tidak bisa berubah tanpa review.

### P0-11 — Dokumentasi rilis, lisensi, dan privasi

- [ ] **P0-11.1 — Tambahkan `README.md`.**
  - Isi minimum: target distro/desktop, dependency, instalasi, first run,
    setup root, uninstall, troubleshooting, dan known limitations.
- [ ] **P0-11.2 — Tambahkan `LICENSE`.**
  - Tentukan lisensi proyek; pastikan kompatibel dengan komponen pihak ketiga.
- [ ] **P0-11.3 — Tambahkan `SECURITY.md` dan `PRIVACY.md`.**
  - Jelaskan model privilege, installer root, data yang disimpan, lokasi log,
    tidak ada telemetri, dan cara melaporkan masalah keamanan.
- [ ] **P0-11.4 — Tambahkan `CHANGELOG.md` dan third-party notices.**
  - Catat `waydroid_script` GPL-3.0, Phantom MIT, dan lisensi font/aset.
- **DoD:** pengguna baru dapat memasang, memahami risiko root, dan melakukan
  troubleshooting tanpa membaca source code.

---

## 3. P1 — Launch hardening

P1 tidak boleh menunda P0, tetapi sebaiknya selesai sebelum rilis publik.

### P1-1 — Kebenaran state dan command frontend

- [ ] Gunakan `CommandResult.code` (`not_found`, `not_initialized`,
  `elevation_denied`, `timeout`, `cancelled`, dan seterusnya) di
  `src/App.jsx`; hapus pencocokan pesan string seperti `includes("No such file")`.
- [ ] Perbaiki `checkStatus()`: saat command error karena Waydroid tidak ada,
  jangan otomatis menyetel `isInitialized = true`; bedakan “belum terpasang”,
  “belum init”, dan “offline”.
- [ ] Ganti `setTimeout` tetap pada start/stop/restart dengan polling
  `waydroid_status_json`/`booted` sampai state benar atau timeout.
- [ ] Cegah polling 5 detik yang tumpang tindih; batalkan request lama dan
  jangan biarkan respons lama menimpa state baru.
- [ ] Jadikan `run_phantom()` async/detached dan verifikasi terminal benar-benar
  ditemukan; saat ini `.output()` dapat menunggu terminal ditutup.
- [ ] Pindahkan `check_host_diagnostics()` dan `open_shared_folder()` dari main
  thread; beri timeout pada `systemctl`/`xdg-open`.
- [ ] Tangani `dirs::data_local_dir() == None` di `open_shared_folder()`; jangan
  membuat path relatif di current directory.
- [ ] Sinkronkan `src/components/terminal/stepConfigs.js` dengan output aktual;
  label “Cloning/Updating” dan “pacman -S” sudah tidak sesuai dengan script
  pinned saat ini.
- [ ] Ambil versi UI dari sumber tunggal (mis. `package.json`/Tauri), jangan
  hardcode `v0.1.0` di `Sidebar.jsx`.
- **DoD:** semua state transisi punya test; tidak ada toast sukses prematur.

### P1-2 — Test, lint, dan E2E frontend

- [ ] Tambahkan ESLint + Prettier dan script `lint`/`format:check`.
- [ ] Tambahkan Vitest + React Testing Library untuk:
  - status backend error/`code`,
  - dialog konfirmasi wipe,
  - install GApps/Houdini/multitouch sukses/gagal,
  - Cancel dan timeout,
  - drag-and-drop APK invalid.
- [ ] Tambahkan smoke E2E dengan WebDriver/Tauri driver untuk startup,
  navigasi tab, dan setup wizard.
- [ ] Masukkan semuanya ke CI.
- **DoD:** `npm run lint` dan `npm test` wajib hijau; alur kritis tidak hanya
  diuji manual.

### P1-3 — Aksesibilitas

- [ ] `AppCard` memakai `div` dengan `onClick`; tambahkan `role`, `tabIndex`,
  dan handler keyboard (`Enter`/`Space`).
- [ ] Preset resolution memakai `role="button"` tanpa `onKeyDown`; lengkapi.
- [ ] Tambahkan focus trap, initial focus, dan restore focus pada
  `ConfirmModal`/`TerminalModal`; saat ini `role="dialog"` saja belum cukup.
- [ ] Pastikan context menu bisa dioperasikan keyboard dan punya `role="menu"`.
- [ ] Audit label ikon, kontras, `aria-live`, dan `prefers-reduced-motion`.
- **DoD:** seluruh alur utama bisa diselesaikan hanya dengan keyboard dan
  screen reader dasar.

### P1-4 — Diagnostik runtime dan first-run

- [ ] Hubungkan `SetupWizard` ke `waydroid_status_json()`; tampilkan langkah
  berbeda untuk Waydroid hilang, belum init, init berjalan, dan siap.
- [ ] Tampilkan preflight P0-7 di UI sebelum tombol install/start.
- [ ] Tambahkan petunjuk actionable untuk: polkit agent tidak ada, sudo butuh
  TTY, `adb` tidak ada, `/dev/kvm` tidak bisa diakses, dan X11.
- [ ] Pastikan `check_host_diagnostics` tidak melakukan blocking call di main
  thread.
- **DoD:** host yang belum siap mendapat penjelasan dan perbaikan, bukan error
  mentah.

### P1-5 — Maintenance dependency dan supply chain

- [ ] Perbarui toolchain frontend. `npm audit` penuh saat ini melaporkan
  **7 dev-only vulnerability (4 high)**, termasuk Vite 5.4.21; upgrade ke Vite
  yang didukung atau dokumentasikan risiko yang diterima.
- [ ] Jalankan `cargo audit`/`cargo deny` dan `npm audit --omit=dev` di CI.
- [ ] Pin GitHub Actions ke SHA dan review permission workflow.
- [ ] Tinjau `core:default` di `capabilities/default.json`; pertahankan hanya
  permission yang dipakai.
- [ ] Batasi pembacaan icon dari file `.desktop`: validasi path, tipe file,
  ukuran maksimum; jangan membaca file arbitrer tanpa batas.
- [ ] Dokumentasikan ADB auto-authorization dan batasi command ke serial
  Waydroid (sudah ada upaya di `adb.rs`, tetapi perlu test/regresi).
- **DoD:** tidak ada temuan high/critical yang tidak tercatat dan diterima
  secara eksplisit.

### P1-6 — Logging dan privasi

- [ ] Redaksi data sensitif dari log: path home, argumen `adb shell` arbitrer,
  identitas device/profil, dan output yang mungkin berisi token.
- [ ] Batasi ukuran log UI (log terminal tidak boleh tumbuh tanpa batas) dan
  batasi output `waydroid_log`.
- [ ] Sediakan UI untuk melihat, mengekspor, dan menghapus log; dokumentasikan
  lokasinya.
- [ ] Pastikan tidak ada telemetry/upload otomatis.
- **DoD:** log cukup untuk support tanpa membocorkan data pribadi.

### P1-7 — Data, config, dan lifecycle

- [ ] Beri versi/skema pada config profil dan migrasi aman; file rusak harus
  fallback tanpa panic.
- [ ] Pastikan penulisan config atomik dan permission benar.
- [ ] Dokumentasikan path config/cache/log, data Waydroid, dan cara uninstall
  bersih.
- [ ] Uji marker install saat image di-wipe/di-init ulang; jangan percaya marker
  basi.
- [ ] Sediakan backup/export sebelum wipe (minimal peringatan eksplisit).
- **DoD:** tidak ada state “installed” palsu dan tidak ada data user yang
  tertinggal tanpa penjelasan.

### P1-8 — Strategi update dan distribusi

- [ ] Pilih: update manual dengan release notes, atau Tauri updater
  bertanda tangan.
- [ ] Jika updater: kelola signing key, endpoint, rollback, dan verifikasi.
- [ ] Jika manual: sediakan cek versi dan instruksi upgrade yang jelas.
- **DoD:** tidak ada update yang mengeksekusi artefak tanpa signature.

### P1-9 — Performa dan UX operasi

- [ ] Kurangi probe berulang: satu `status_json`/snapshot per polling, cache
  app list/icon berdasarkan mtime.
- [ ] Tampilkan progress tahap yang benar (sinkron dengan script).
- [ ] Pastikan tombol disabled selama operasi dan bisa pulih setelah
  gagal/cancel.
- [ ] Ukur startup, memory, dan waktu buka daftar app; tetapkan budget.
- **DoD:** UI tidak freeze saat `systemctl`/`xdg-open`/terminal dibuka dan
  polling tidak menumpuk.

### P1-10 — Ketahanan installer

- [ ] Uji instalasi ulang GApps/Houdini/multitouch setelah sukses, gagal di
  tengah, dan marker dihapus.
- [ ] Uji disk penuh/network putus dan pastikan error jelas serta tidak merusak
  image.
- [ ] Tambahkan verifikasi post-install (live status) sebelum marker dianggap
  valid.
- **DoD:** operasi idempoten atau gagal dengan aman; tidak ada marker palsu.

---

## 4. P2 — Pasca-rilis / polish

Tidak menjadi blocker rilis; kerjakan setelah P0/P1 stabil.

- [ ] Fitur lanjutan dari [`docs/BACKEND-TODO.md`](docs/BACKEND-TODO.md):
  file transfer/backup, primitive input/keymapping, model mapping, makro,
  `--root`, `globalsetting`, `show-full-ui`, `intent`, `logcat`, dan
  `container freeze/unfreeze`.
- [ ] Dukungan multi-distro penuh jika rilis awal Arch-only.
- [ ] Auto-update, crash report opt-in, dan diagnostic bundle.
- [ ] Paket distribusi tambahan (AUR/Flathub) dan tanda tangan repository.
- [ ] i18n (UI saat ini Inggris), tema, onboarding, dan dokumentasi screenshot.
- [ ] Optimasi daftar app besar, cache ikon, dan virtualisasi log.
- [ ] Perbaiki favicon `/vite.svg`, metadata versi, dan detail visual kecil.
- [ ] Perluas test matrix desktop (GNOME/KDE/Hyprland, Wayland/X11, GPU
  Intel/AMD/NVIDIA).

---

## 5. Risiko residual yang harus disadari

| Risiko | Status | Mitigasi |
|---|---|---|
| Waydroid hanya satu container | Diterima | Dokumentasikan; jangan janjikan multi-instance. |
| `waydroid_script` berjalan sebagai root | Terbuka | P0-6: hash-pin dependency, audit, atau keluarkan dari build produksi. |
| Distro target belum jelas | Terbuka | P0-7: pilih Arch-only atau multi-distro secara eksplisit. |
| Belum ada uji live Waydroid/polkit | Terbuka | P0-8: VM disposable + bukti uji. |
| CSP masih `null` dan font remote | Terbuka | P0-5. |
| Log dapat memuat argumen sensitif | Terbuka | P1-6. |
| Vite 5 dev dependency rentan | Terbuka | P1-5; tidak masuk runtime bundle, tetapi build tooling. |
| Tanpa telemetry | Diterima | Support mengandalkan log yang diekspor pengguna. |

---

## 6. Urutan eksekusi yang disarankan

1. **Tentukan scope platform** (Arch-only vs multi-distro) dan siapkan VM uji.
2. **P0-3 + P0-4**: hasil privileged, timeout, cancel, task isolation.
3. **P0-5 + P0-6**: CSP/offline dan supply chain installer root.
4. **P0-7 + P0-8**: preflight dependency dan validasi live end-to-end.
5. **P0-9 + P0-10 + P0-11**: artefak, CI remote, dokumentasi/legal.
6. **P1**: frontend reliability, test/lint/E2E, a11y, logging, data lifecycle.
7. **P2**: fitur dan polish.

---

## 7. Gate akhir rilis

Rilis hanya boleh diberi label production-ready jika:

- [ ] Semua item **P0** di atas selesai.
- [ ] Ada bukti uji live untuk alur utama dan failure path di VM bersih.
- [ ] CSP aktif, tidak ada request eksternal yang tidak disengaja, dan aplikasi
  berfungsi offline.
- [ ] Tidak ada installer root yang memakai dependency unpinned.
- [ ] Artefak yang dipublikasikan sudah diuji install/uninstall dan punya
  checksum/signature.
- [ ] CI di remote hijau dan menjadi required check.
- [ ] README, LICENSE, SECURITY, PRIVACY, CHANGELOG, dan third-party notices
  tersedia.
- [ ] Known limitations dan residual risk didokumentasikan, bukan disembunyikan.
- [ ] Tidak ada bug P0/P1 terbuka yang belum diterima secara eksplisit.

**Jangan menyebut production-ready hanya karena build `.deb`, `cargo test`, dan
`npm run build` lulus.**
