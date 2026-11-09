# ArchDroid — Rencana Improvement

> **Dibuat:** 9 Nov 2026, dari pembacaan kode di branch `feat/touch-mode`
> (`d66aa50`, 2 commit di depan `main`).
> **Hubungan dengan dokumen lain:** [`docs/archive/RELEASE-GATE-0.1.md`](docs/archive/RELEASE-GATE-0.1.md) (dulu `RELEASE-GATE-0.1.md`) adalah gate rilis 0.1.x
> (semua P0/P1 selesai). [`docs/BACKEND-TODO.md`](docs/BACKEND-TODO.md) adalah
> backlog fitur backend. Rencana ini tidak mengulang isi keduanya; item yang
> sudah tercatat di sana hanya dirujuk.
> **Effort:** `S` ≤ sehari · `M` beberapa hari · `L` ~1 minggu (skala yang sama
> dengan BACKEND-TODO).
> **Prioritas:** `P1` sebelum/bersama rilis berikutnya · `P2` beberapa minggu ke
> depan · `P3` saat menyentuh area itu atau bila ada permintaan.


## Status pengerjaan

Branch `improve/plan-0.1.6` (dari `feat/touch-mode`), 9 Nov 2026, PR #2. CI dan workflow paket hijau di GitHub. `cargo audit` di CI menangkap 4 advisory pada `zip` 4.6.1 → diganti `zip` 9 (Rust minimum 1.88). Setiap item
satu commit atau lebih; semua test, lint, clippy, dan E2E hijau setelah tiap
commit.

| Item | Status |
|---|---|
| A1, A2, A3 | ✅ Selesai, dengan test yang gagal sebelum perbaikan |
| A4 | ✅ Selesai: root hanya saat `PermissionDenied` |
| A5 | ✅ Terbukti lebih parah dari dugaan: `ro.sf.lcd_density` di-set saat boot, `prop set` ditolak, jadi preset DPI **selalu** gagal. Diganti `wm density` (diuji live: langsung berlaku, bertahan setelah restart). Resolusi (`persist.waydroid.width/height`) diuji live dan bekerja |
| A6 | ✅ Selesai: tahap init digabung; status awal `null` |
| B1 | ⏳ Tindakan maintainer: merge, smoke rilis, tanda tangan, publikasi |
| B2 | ✅ (1) `.deb`/`.rpm` dicek glibc 2.35. (2) Workflow membangun AppImage, `.deb`, `.rpm` dan paket Arch (job container); hijau di GitHub. Tool AppImage (AppRun, linuxdeploy) di-prefetch lewat API dengan checksum karena URL rilis Tauri 504 pada 9 Nov |
| B3 | ✅ Selesai (termasuk `Cargo.lock`) |
| C1 | ✅ Selesai; `npm audit` penuh kini 0, risiko dev-only ditutup |
| C2, C3 | ✅ Selesai; CI hijau di GitHub (PR #2) |
| D1 | ✅ Selesai: `src/lib/tasks.js`; `App.jsx` 826 → ±590 baris |
| D2 | ✅ Versi ringan: parser baris di frontend (`src/lib/waydroidStatus.js`) alih-alih mengubah format wire, supaya mock test dan E2E tidak berubah |
| D3 | ✅ Selesai |
| E2 | ✅ Selesai: command `waydroid_boot_state`, "Android Booting…" |
| B4 | ✅ `packaging/aur/PKGBUILD` diuji dengan `makepkg` terhadap v0.1.5 (signature + checksum lolos). Publikasi ke AUR: tindakan maintainer |
| C4 | ✅ `scripts/e2e/real-binary-smoke.py` lulus di GitHub Actions (PR #2): 9/9 cek (jendela, IPC, CSP memblokir eval/inline/fetch/script remote) |
| D4 | ✅ Langkah pertama: `logs.rs` (area yang disentuh A4). Area lain menunggu disentuh |
| D5 | ✅ `HostFacts::probe()` + `report()` murni; 4 test tabel; laporan host identik sebelum/sesudah |
| F5 | ✅ Sebagian: hardware ini diukur (jendela 212–402 ms; PSS ±290 MB, budget baru PSS < 350 MB). GNOME/KDE/AMD tetap butuh mesin lain |
| E1 | ⏳ Tindakan maintainer: kirim issue/PR ke upstream |
| E3 | ⏳ Dicoba 9 Nov dengan harness aman (jendela fullscreen milik harness menutup kedua monitor sebelum satu sentuhan pun dikirim): Hyprland 0.56 mengenali touchscreen virtual (`archdroid-touch-probe`, juga `ArchDroid Touch` dari kode touch mode), tetapi jendela GTK3 maupun GTK4 harness tidak menerima event sentuh apa pun, jadi pemetaan monitor belum bisa diamati. Langkah berikut: harness dengan klien Wayland yang terbukti menerima `wl_touch` (mis. `wev`), atau uji langsung dengan Waydroid di DP-2 bersama pengguna. Kode belum diubah |
| E4 | ✅ `.apks`/`.xapk`/`.apkm` lewat `adb install-multiple` (`src-tauri/src/apk.rs`, crate `zip` 4.6.1 deflate-only). Diuji live: ARCore (4 split) terpasang ulang dari `.apks` dan `.xapk` bersarang; bundle tanpa base ditolak dengan alasan Android. OBB ditolak dengan petunjuk (belum didukung) |
| E5 | ⏳ Spike selesai, keputusan maintainer: [`docs/BACKUP-DESIGN.md`](docs/BACKUP-DESIGN.md). Diuji live: APK bisa dicadangkan tanpa root, data app tidak (`adb root` tidak tersedia; data di host milik uid app, mode 700). Opsi 1 (backup APK tanpa root, "Save APK") **sudah dikerjakan** dan diuji live (simpan ARCore → pasang ulang). Opsi 2/3 (data) menunggu keputusan |
| F1, F2, F3, F4 | ✅ Selesai |

Item baru dari arsip gate (P2 yang belum tercakup): optimasi daftar app
besar dan virtualisasi log terminal; kerjakan hanya bila ada laporan lambat.

---

## 1. Kondisi saat ini

Diukur ulang 9 Nov 2026:

| Area | Hasil |
|---|---|
| Rust | `cargo test`: 163 lulus, 2 `#[ignore]` (test live touch mode). `cargo clippy --all-targets -- -D warnings` dan `cargo fmt --check` bersih. |
| Frontend | `node --test`: 43 lulus, 1 skip (`dist-release` belum di-stage). Vitest: 72 lulus. ESLint dan Prettier bersih. |
| Rilis | `v0.1.5` (3 Nov). `main` membawa `[Unreleased]` (Control Center, Force stop, "Waydroid Not Responding"); touch mode masih di `feat/touch-mode`. |
| File terbesar | `src-tauri/src/lib.rs` 2052 baris (±1790 kode produksi), `src/index.css` 3176, `touchmode.rs` 1228, `src/App.jsx` 807. |

Yang sudah kuat dan **tidak** perlu ditulis ulang: batas privilege (consent,
approval paket, workdir `0700`), single-flight `TaskGuard` dengan cancel per
`task_id`, CSP, pinning commit/hash/SHA-256, matriks uji live di VM, dan
kebiasaan "klaim README mengikuti bukti uji". Rencana ini fokus pada celah yang
ditemukan di kode.

---

## 2. Ringkasan

| ID | Item | Prio | Effort |
|---|---|---|---|
| A1 | Retry tidak melakukan apa pun untuk "Enable access" touch mode | P1 | S |
| A2 | Status multitouch tidak di-refresh setelah install dan saat startup | P1 | S |
| A3 | Settings membaca properti sekali saat mount; toggle aktif saat engine mati | P1 | S |
| A4 | "View Logs" meminta root untuk file yang bisa dibaca user | P1 | S |
| A5 | Preset DPI memakai `ro.sf.lcd_density` saat runtime (perlu verifikasi) | P1 | S–M |
| A6 | Polish kecil: tahap stepper yang tak pernah tercapai, kilasan UI saat first run | P3 | S |
| B1 | Rilis 0.1.6 | P1 | M |
| B2 | Guard glibc untuk `.deb`/`.rpm`, lalu build semua paket di CI | P1 / P2 | S / M |
| B3 | Guard versi sinkron di 4 file | P1 | S |
| B4 | Paket AUR `archdroid-bin` sebagai jalur update Tier 1 | P2 | S–M |
| C1 | `eslint-plugin-react-hooks` | P1 | S |
| C2 | Cache build Rust dan `cargo-audit` prebuilt di CI | P2 | S |
| C3 | Dependabot untuk npm, cargo, dan GitHub Actions | P2 | S |
| C4 | Smoke test biner asli (WebKitGTK) di CI | P2 | M |
| D1 | Satu tabel "root action" di `App.jsx` (konfirmasi dan Retry) | P2 | M |
| D2 | Status Waydroid terstruktur dari backend | P2 | M |
| D3 | Hapus heuristik teks di `stepConfigs.js` | P3 | S |
| D4 | Pecah `lib.rs` per area saat area itu disentuh | P3 | M |
| D5 | `diagnostics.rs` bisa diuji per cek | P3 | M |
| E1 | Kirim patch upstream `fake_touch` | P2 | S (kirim) / L (build image) |
| E2 | Status "Booting" vs "Ready" | P2 | M |
| E3 | Touch mode: touchpad dan multi-monitor | P3 | M–L |
| E4 | Install split APK (`.apks`/`.xapk`/`.apkm`) | P2 | M |
| E5 | Backup data app sebelum Factory Reset | P3 | L |
| F1 | Terjemahkan `SECURITY.md` ke bahasa Inggris | P1 | S |
| F2 | Kalimat akses `/dev/uinput` di README dibuat tepat | P1 | S |
| F3 | Issue template dan `CONTRIBUTING.md` | P2 | S |
| F4 | Arsipkan `RELEASE-GATE-0.1.md`; satu backlog yang hidup | P3 | S |
| F5 | Matriks hardware (GNOME, KDE, AMD) dan ukur startup/RSS | P2 | M |

---

## 3. A — Bug dan kebenaran perilaku

Kerjakan duluan. Setiap perbaikan dimulai dengan test yang gagal.

### A1 — Retry untuk "Enable access" tidak melakukan apa pun (P1, S)
- **Bukti:** `executeEnableTouchAccess` menjalankan task dengan
  `taskType: "touch-access"` (`src/App.jsx:471-474`). `handleRetryTask`
  (`src/App.jsx:620-657`) tidak punya `case "touch-access"`. Tombol Retry tetap
  tampil karena `isFailed` diturunkan dari `taskResult`, juga untuk task tanpa
  tahap (`src/components/terminal/stepConfigs.js:135-149`,
  `src/components/terminal/TerminalModal.jsx:376`).
- **Dampak:** setelah prompt polkit ditolak atau gagal, Retry tidak berbuat apa-apa.
- **Kerjakan:** tambahkan `touch-access` ke daftar yang menutup terminal dan ke
  `switch` (memanggil `handleEnableTouchAccess`). Perbaikan strukturalnya di D1.
- **DoD:** test Vitest: task `touch-access` yang gagal → Retry membuka lagi dialog
  konfirmasi.

### A2 — Status multitouch basi (P1, S)
- **Bukti:** `onTaskSuccess` hanya me-refresh GApps, Houdini, dan daftar app
  (`src/App.jsx:71-75`); effect saat mount juga hanya GApps dan Houdini
  (`src/App.jsx:107-111`). Multitouch hanya di-refresh di `onOnline`
  (`src/App.jsx:41-45`).
- **Dampak:** setelah "Declare Touch Features" sukses, kartu di Install Tools
  masih menampilkan status lama sampai engine di-restart. Bila engine mati saat
  app dibuka, kartu tetap `unknown`, padahal backend bisa menjawab dari file
  overlay/marker tanpa sesi (`src-tauri/src/lib.rs:1011-1018`).
- **Kerjakan:** panggil `checkMultitouchStatus()` di effect mount dan di
  `onTaskSuccess`.
- **DoD:** test: setelah `install_multitouch` sukses, `check_multitouch_status`
  dipanggil lagi dan kartu menampilkan status baru.

### A3 — Properti di Settings dibaca sekali saja (P1, S)
- **Bukti:** `loadConfigs` hanya dipanggil saat mount
  (`src/hooks/useDeviceConfig.js:35-61`). `waydroid prop get/set` butuh sesi
  yang berjalan, padahal engine biasanya belum dinyalakan saat app dibuka.
  Toggle Multi-Window dan Invert Colors hanya di-disable oleh `busy`, bukan oleh
  status engine (`src/components/settings/SettingsView.jsx:93-121`).
- **Dampak:** Settings menampilkan nilai default (OFF, resolusi kosong) walau
  properti sudah di-set. Menekan toggle saat engine mati berakhir dengan toast
  "Config Toggle Failed".
- **Kerjakan:** panggil `loadConfigs` lagi saat status berubah ke `online` (lewat
  `onOnline` yang sudah ada). Saat offline, disable toggle dan tulis bahwa engine
  harus berjalan.
- **DoD:** test hook: mount saat offline → online → `waydroid_prop_get` dipanggil
  ulang dan state diperbarui; toggle disabled saat offline.

### A4 — "View Logs" tanpa root bila file bisa dibaca (P1, S)
- **Bukti:** `waydroid_log` selalu lewat `stream_privileged_script` hanya untuk
  `tail` (`src-tauri/src/lib.rs:569-583`). Di host pengembang,
  `/var/lib/waydroid/waydroid.log` ber-mode `0644 root:root`, dan Diagnostic
  Bundle sudah membacanya tanpa root (`log_tail_or_reason`,
  `src-tauri/src/bundle.rs:147`).
- **Dampak:** prompt polkit untuk aksi baca-saja, bertentangan dengan prinsip
  least-privilege di README.
- **Kerjakan:** bila file bisa dibaca, jalankan `tail` lewat `stream_user_script`
  (event dan terminal tetap sama). Root hanya dipakai saat `PermissionDenied`.
- **DoD:** unit test pemilihan jalur (bisa dibaca → user, `PermissionDenied` →
  privileged); uji live: View Logs tanpa prompt pada host dengan log `0644`.

### A5 — Preset DPI mungkin tidak pernah berlaku (P1, S untuk verifikasi)
- **Bukti:** preset menulis DPI ke `ro.sf.lcd_density` lewat `waydroid prop set`
  (`src/hooks/useDeviceConfig.js:9`, `:108-127`), lalu toast menyuruh restart
  sesi. Properti `ro.*` hanya bisa di-set sekali per boot, tidak disimpan
  melewati reboot, dan SurfaceFlinger membacanya saat boot. `prop_set` memang
  memverifikasi dengan membaca balik (`src-tauri/src/waydroid.rs:358-373`);
  test-nya bahkan memakai `ro.sf.lcd_density` sebagai contoh properti yang
  ditolak (`waydroid.rs:614-629`). Di host pengembang, `waydroid.cfg` dan
  `waydroid_base.prop` tidak berisi density.
- **Dugaan dampak:** DPI ditolak bila properti sudah ada, atau hilang setelah
  restart yang diminta toast.
- **Verifikasi dulu:** dengan sesi berjalan: set DPI dari UI, cek
  `waydroid prop get ro.sf.lcd_density` dan `adb shell wm density`, restart sesi,
  cek lagi.
- **Kerjakan bila terbukti (M):** pakai jalur yang sudah ada di balik
  `dev-commands`: `adb shell wm density N` (`src-tauri/src/lib.rs:1316`,
  `src-tauri/src/device.rs:75-77`). Daftarkan ke build produksi hanya bagian
  yang dipakai UI, bersama test-nya (aturan di `lib.rs:1732-1735`).
- **DoD:** DPI dari preset masih berlaku setelah restart; toast sukses hanya
  muncul bila nilainya terbaca balik.

### A6 — Polish kecil (P3, S)
- Stepper init punya tahap "Initialize" (`stepConfigs.js:64`) yang tidak pernah
  dilaporkan: script init hanya mencetak `[STEP] pkg` dan `[STEP] download`
  (`src-tauri/src/lib.rs:846-851`), karena `waydroid init` mengunduh dan
  mengonfigurasi dalam satu perintah. Gabungkan atau hapus tahap itu.
- `isInitialized` mulai dari `true` (`src/hooks/useEngineStatus.js:11`), jadi
  pengguna baru sekilas melihat My Apps kosong sebelum Setup Wizard. Mulai dari
  `null` dan tampilkan placeholder sampai status pertama terbaca.

---

## 4. B — Rilis dan distribusi

### B1 — Rilis 0.1.6 (P1, M)
- **Bukti:** `[Unreleased]` di `CHANGELOG.md` berisi layout baru, touch mode,
  Force stop, dan pesan "Waydroid Not Responding"; `feat/touch-mode` belum
  di-merge ke `main`.
- **Kerjakan:** selesaikan A1–A4, A5 (bila terbukti), B3, C1, F1, dan F2; merge
  `feat/touch-mode`; jalankan smoke rilis seperti 0.1.5 (Arch, Ubuntu 24.04,
  Fedora 44, hardware asli dengan Hyprland), termasuk touch mode dari paket
  (aturan udev terpasang oleh paket, bukan tombol "Enable access").
- **DoD:** bagian smoke 0.1.6 di `docs/P0-8.2-MATRIX-RESULTS.md`; `SHA256SUMS.asc`
  terverifikasi.

### B2 — Guard glibc untuk `.deb`/`.rpm`, lalu build di CI (P1 S / P2 M)
- **Bukti:** `generate-release.sh` hanya menjalankan `check-glibc.sh` untuk
  AppImage (`scripts/release/generate-release.sh:37`). `.deb`, `.rpm`, dan
  `.pkg.tar.zst` dibangun di mesin maintainer (`docs/RELEASE-AND-UPDATES.md`,
  bagian "Building a release").
- **Dampak:** batas glibc biner di `.deb`/`.rpm` bergantung pada mesin build dan
  tidak dicek. Dependency baru yang mengompilasi kode C bisa diam-diam menaikkan
  batasnya dan memecah Debian 12 (glibc 2.36), seperti yang terjadi pada AppImage
  0.1.3.
- **Kerjakan:** (1) S: jalankan `check-glibc.sh` pada isi `.deb`/`.rpm` di
  `generate-release.sh` (skripnya sudah menerima direktori). (2) M: bangun
  `.deb`/`.rpm` di workflow AppImage (Ubuntu 22.04) dan `.pkg.tar.zst` di job
  container `archlinux:base-devel`, masing-masing dengan `BUILD-INFO.txt`.
  Penandatanganan tetap lokal; kunci privat tidak pernah masuk CI.
- **DoD:** rilis menolak paket di atas batas glibc yang dipilih; setiap artefak
  punya commit asal yang tercatat.

### B3 — Versi tetap sinkron (P1, S)
- **Bukti:** versi ditulis di 4 tempat: `package.json:4`,
  `src-tauri/Cargo.toml:3`, `src-tauri/tauri.conf.json:5`,
  `packaging/arch/PKGBUILD:3`. Belum ada test yang membandingkannya
  (`package-metadata.test.js` hanya memeriksa lisensi dan repo).
- **Kerjakan:** test kecil di `src/lib/release-hygiene.test.js` yang
  membandingkan keempatnya.
- **DoD:** bump versi yang lupa satu file gagal di CI.

### B4 — Paket AUR `archdroid-bin` (P2, S–M)
- **Bukti:** tidak ada auto-updater (keputusan yang benar, lihat
  `docs/RELEASE-AND-UPDATES.md`), jadi pengguna Arch hanya tahu ada versi baru
  bila membuka halaman Releases. AUR/Flathub tercatat sebagai P2 di `RELEASE-GATE-0.1.md` §4.
- **Kerjakan:** PKGBUILD AUR yang mengunduh artefak rilis bertanda tangan dan
  memverifikasinya (`validpgpkeys` dengan fingerprint rilis). Update AUR menjadi
  satu langkah di checklist rilis.
- **DoD:** `yay -S archdroid-bin` memasang versi terbaru; helper AUR menampilkan
  update berikutnya.

---

## 5. C — CI dan tooling

### C1 — Lint aturan hooks (P1, S)
- **Bukti:** `eslint.config.js` hanya memakai `eslint-plugin-react`; aturan hooks
  tidak aktif. Kode bergantung pada ref untuk menghindari closure basi
  (`src/hooks/useEngineStatus.js:15-18`, `src/hooks/useStreamingTask.js:34-35`),
  dan effect mount di `src/App.jsx:107-111` memanggil fungsi yang dibuat ulang
  setiap render dengan deps `[]`.
- **Kerjakan:** tambah `eslint-plugin-react-hooks`: `rules-of-hooks` = error,
  `exhaustive-deps` = warn. Perbaiki peringatan yang nyata; beri komentar pada
  yang disengaja.
- **DoD:** `npm run lint` bersih dengan plugin aktif di CI.

### C2 — CI lebih cepat (P2, S)
- **Bukti:** `.github/workflows/ci.yml` tidak meng-cache build Rust (hanya
  `cache: npm`, baris 20), menjalankan clippy dan test dua kali (default dan
  `dev-commands`), dan `cargo install cargo-audit` mengompilasi cargo-audit dari
  source di setiap run (baris 55).
- **Kerjakan:** cache Rust (mis. `Swatinem/rust-cache`) dan cargo-audit sebagai
  biner prebuilt (mis. `taiki-e/install-action`), keduanya di-pin ke SHA seperti
  action lain.
- **DoD:** durasi CI turun; tidak ada action tanpa pin SHA.

### C3 — Dependabot (P2, S)
- **Bukti:** tidak ada `.github/dependabot.yml`. Actions di-pin ke SHA, tetapi
  tidak ada yang memberi tahu saat pin tertinggal. `docs/DEPENDENCY-SECURITY.md`
  menerima 6 temuan moderate dev-only dengan catatan "re-check".
- **Kerjakan:** Dependabot mingguan untuk `npm`, `cargo` (direktori
  `src-tauri`), dan `github-actions`, dengan update yang dikelompokkan.
- **DoD:** PR update otomatis lewat CI yang sama.

### C4 — Smoke test biner asli di CI (P2, M)
- **Bukti:** `scripts/e2e/smoke-webdriver.js` menjalankan `dist/` di Chromium
  headless dengan `__TAURI_INTERNALS__` tiruan (baris 49-53 dan 70). Backend
  Rust, WebKitGTK, dan CSP di dalam Tauri tidak teruji di CI; semua itu baru
  diverifikasi manual di VM (WebKitWebDriver, 1 Nov).
- **Kerjakan:** job yang membangun biner rilis dan menjalankannya di Xvfb dengan
  `tauri-driver` + `WebKitWebDriver`: jendela terbuka, nol
  `securitypolicyviolation`, semua tab bisa dibuka, `check_host_diagnostics`
  menjawab. Pakai ulang `scripts/test-vm/live/csp_probe.py` bila cocok.
- **DoD:** job hijau di `main`; gagal start atau pelanggaran CSP memecah CI.

---

## 6. D — Maintainability (mencegah kelas bug)

### D1 — Satu tabel "root action" (P2, M)
- **Bukti:** `src/App.jsx` punya enam alur "konfirmasi → `runStreamingTask` →
  toast error" yang hampir sama (GApps, Houdini, multitouch, touch access, init,
  wipe; `src/App.jsx:298-618`), plus `handleRetryTask` yang harus disinkronkan
  manual (`:620-657`). A1 adalah bukti bahwa keduanya sudah tidak sinkron.
- **Kerjakan:** satu tabel per `taskType` (command, judul, isi konfirmasi,
  preview paket, refresh setelah sukses) yang dipakai tombol dan Retry;
  `handleRetryTask` menjadi satu lookup.
- **DoD:** task root baru cukup ditambah satu entri; test memastikan setiap
  `taskType` yang bisa gagal punya jalur Retry.

### D2 — Status Waydroid terstruktur (P2, M)
- **Bukti:** frontend menebak status dengan `res.data.includes("RUNNING")` di dua
  tempat (`src/hooks/useEngineStatus.js:44`, `:78`), padahal backend sudah punya
  parser (`waydroid::session_running`, `container_frozen`) dan `StatusSnapshot`,
  tetapi hanya di build `dev-commands` (`src-tauri/src/lib.rs:1095-1150`).
- **Dampak:** dua parser untuk satu format; UI tidak bisa membedakan "Frozen"
  atau "Booting" dari "Online" (lihat E2).
- **Kerjakan:** `waydroid_status` mengembalikan objek kecil
  `{ session, container, frozen }`, lalu pencocokan teks di frontend dihapus.
  Jangan tambahkan probe adb ke poll 5 detik.
- **DoD:** tidak ada `includes("RUNNING")` di `src/`; kontrak baru tercakup test
  backend dan `src/lib/ipc-surface.test.js`.

### D3 — Hapus heuristik teks di stepper (P3, S)
- **Bukti:** semua script yang punya stepper mencetak `[STEP] <id>`
  (`src-tauri/src/scripts.rs`, `lib.rs:847-848`), dan `detectStepProgress`
  memakai marker itu lebih dulu. Cabang heuristik teks
  (`src/components/terminal/stepConfigs.js:176-291`, ±115 baris) hanya berjalan
  sebelum marker pertama muncul, saat tahap aktif masih tahap 0 (nilai default).
- **Kerjakan:** hapus cabang itu setelah `stepConfigs.test.js` dipastikan hanya
  bergantung pada marker; deteksi `[ERROR]` dan cancel dipertahankan.
- **DoD:** test stepper tetap lulus tanpa cabang heuristik.

### D4 — Pecah `lib.rs` per area (P3, M, bertahap)
- **Bukti:** `lib.rs` mencampur daftar app dan ikon (`:297-400`), validasi APK
  (`:402-531`), log dan redaksi (`:569-694`), status add-on (`:884-1052`),
  Phantom (`:1554-1688`), dan env AppImage (`:1690-1712`). Modul lain sudah
  terpisah rapi.
- **Kerjakan:** pindahkan satu area ke modulnya sendiri (`apps.rs`, `apk.rs`,
  `logs.rs`, `addons.rs`, `phantom.rs`) **hanya saat area itu diubah**, tanpa
  perubahan perilaku, test ikut pindah. `lib.rs` tersisa command tipis dan
  `run()`.
- **DoD:** setiap pemindahan adalah commit `refactor:` tersendiri dengan test yang
  sama tetap lulus.

### D5 — `diagnostics.rs` bisa diuji per cek (P3, M)
- **Bukti:** 17 cek host (`collect_diagnostics`, `src-tauri/src/diagnostics.rs:8`)
  dengan teks remediation yang dibaca pengguna, tetapi hanya 3 unit test di file
  itu; sebagian besar cek membaca sistem secara langsung.
- **Kerjakan:** pisahkan pengumpulan fakta (struct input) dari keputusan status
  dan remediation (fungsi murni), seperti `bundle::render(BundleInput)`; lalu
  test tabel per cek.
- **DoD:** setiap cek punya test untuk setiap status yang bisa dihasilkannya.

---

## 7. E — Fitur

### E1 — Kirim patch upstream `fake_touch` (P2, S kirim / L build image)
- **Bukti:** `docs/upstream/waydroid-fake-touch.md` berisi teks issue dan PR yang
  siap dikirim; patch belum di-build ke image.
- **Dampak:** perbaikan di Waydroid berlaku di semua compositor (touch mode hanya
  Hyprland) dan tidak butuh aturan udev `/dev/uinput`.
- **Kerjakan:** kirim issue dan PR sekarang dengan catatan "belum di-build"; build
  image sendiri hanya bila maintainer upstream memintanya. Setelah upstream
  merilisnya, panel Touch Mode menyarankan `fake_touch` di compositor selain
  Hyprland.
- **DoD:** tautan issue/PR dicatat di dokumen upstream itu.

### E2 — Status "Booting" vs "Ready" (P2, M)
- **Bukti:** status menjadi `online` begitu sesi `RUNNING`
  (`src/hooks/useEngineStatus.js:41-50`), sebelum Android selesai boot.
  `adb::booted()` (cek `sys.boot_completed`) sudah ada, tetapi hanya di
  `dev-commands` (`src-tauri/src/adb.rs:248-253`).
- **Dampak:** Launch dan Install saat boot pertama bisa lambat atau gagal tanpa
  penjelasan; daftar app kosong selama boot pertama (CHANGELOG 0.1.3).
- **Kerjakan:** setelah Start, cek `sys.boot_completed` hanya sampai bernilai `1`
  (bukan setiap 5 detik); top bar menampilkan "Booting…" dan Launch menunggu.
  Bergantung pada D2.
- **DoD:** test hook untuk transisi offline → booting → online.

### E3 — Touch mode: touchpad dan multi-monitor (P3, M–L)
- **Bukti:** batasan tercatat di `docs/BACKEND-TODO.md` §5: touchpad belum,
  pemetaan multi-monitor hanya monitor yang fokus, rotasi output belum.
  Strukturnya sudah siap: trait `Probe` (`src-tauri/src/touchmode.rs:207`)
  dengan implementasi Hyprland (`:333`).
- **Kerjakan:** tap touchpad sebagai sentuhan dan pemetaan per monitor. Backend
  compositor lain (mis. skrip KWin) hanya bila E1 macet.
- **DoD:** unit test pemetaan koordinat multi-monitor dan rotasi; test live
  `#[ignore]` per kasus.

### E4 — Install split APK (P2, M)
- **Bukti:** `apk_looks_installable` hanya menerima satu ZIP berisi
  `AndroidManifest.xml`, dan install hanya memasang satu file
  (`src-tauri/src/lib.rs:412-457`); drag-and-drop hanya menerima `.apk`.
- **Dampak:** banyak game didistribusikan sebagai split APK (`.apks`, `.xapk`,
  `.apkm`) dan belum bisa dipasang dari ArchDroid.
- **Kerjakan:** pastikan dulu ada permintaan (issue). Lalu ekstrak ke direktori
  privat, validasi setiap split, dan pasang dengan `adb install-multiple`; tanpa
  adb, tolak dengan pesan yang jelas. OBB bisa menyusul.
- **DoD:** test validasi arsip (path traversal, batas ukuran), test argv
  `install-multiple`, dan uji live dengan satu game split.

### E5 — Backup data app sebelum Factory Reset (P3, L)
- **Bukti:** dialog wipe hanya menyarankan menyalin file secara manual lewat
  Shared Storage (`src/App.jsx:597`). Backup/restore ada di
  `docs/BACKEND-TODO.md` §4, belum dikerjakan.
- **Kerjakan:** mulai dari backup per app (`/data/data/<pkg>` + APK) sesuai §4,
  lalu tawarkan di dialog wipe.
- **DoD:** mengikuti DoD di BACKEND-TODO §4.

---

## 8. F — Dokumentasi dan komunitas

### F1 — `SECURITY.md` dalam bahasa Inggris (P1, S)
- **Bukti:** `SECURITY.md` seluruhnya berbahasa Indonesia, sementara README,
  PRIVACY, CHANGELOG, VERIFY-RELEASE, dan RELEASE-AND-UPDATES sudah
  diterjemahkan ke bahasa Inggris di 0.1.4.
- **Dampak:** pelapor kerentanan dari luar Indonesia sulit membaca kebijakan dan
  cara melapor.
- **Kerjakan:** terjemahkan tanpa mengubah isi (model privilege, touch mode,
  kanal laporan).

### F2 — Kalimat akses `/dev/uinput` di README (P1, S)
- **Bukti:** README menulis "use `/dev/uinput` and read mice (never keyboards)"
  (`README.md:156`). Akses tulis ke `/dev/uinput` memungkinkan proses apa pun
  milik user membuat perangkat input virtual, termasuk keyboard. `SECURITY.md`
  sudah menyebut konsekuensi ini; README belum.
- **Kerjakan:** satu kalimat: ArchDroid tidak pernah membaca keyboard, tetapi
  `/dev/uinput` memungkinkan proses milik user membuat perangkat input virtual
  (akses yang sama dengan Steam Input), plus cara mencabutnya.

### F3 — Issue template dan `CONTRIBUTING.md` (P2, S)
- **Bukti:** `.github/` hanya berisi workflow; tidak ada issue template atau
  `CONTRIBUTING.md`. Tanpa telemetry, dukungan bergantung pada Diagnostic Bundle
  yang dilampirkan pengguna (`RELEASE-GATE-0.1.md` §5).
- **Kerjakan:** `bug_report.yml` (versi, distro, compositor, GPU, Diagnostic
  Bundle), `config.yml` yang mengarahkan laporan keamanan ke Security Advisories,
  dan `CONTRIBUTING.md` berbahasa Inggris: build/test, aturan kerja backend
  (BACKEND-TODO), Conventional Commits, dan fitur `dev-commands`.

### F4 — Arsipkan `RELEASE-GATE-0.1.md` (P3, S)
- **Bukti:** `RELEASE-GATE-0.1.md` (80 KB) adalah gate rilis 0.1.x yang sudah tercapai; item
  P2 yang tersisa tumpang tindih dengan BACKEND-TODO dan rencana ini. Tidak ada
  dokumen lain yang menautkannya.
- **Kerjakan:** pindahkan ke `docs/archive/RELEASE-GATE-0.1.md`; item P2 yang
  masih relevan disalin ke rencana ini.

### F5 — Matriks hardware dan budget performa (P2, M)
- **Bukti:** README (Known Limitations #5): GNOME, KDE, dan GPU AMD belum diuji.
  Budget startup (< 1,5 s) dan RSS (< 200 MB) dari `RELEASE-GATE-0.1.md` P1-9 belum pernah
  diukur.
- **Kerjakan:** satu sesi uji per desktop (GNOME, KDE) dan satu mesin AMD; ukur
  waktu sampai jendela tampil dan RSS dengan biner rilis; catat di
  `docs/P0-8.2-MATRIX-RESULTS.md`.

---

## 9. Sengaja tidak direkomendasikan sekarang

- **Auto-updater:** keputusan di `docs/RELEASE-AND-UPDATES.md` tetap benar (app
  meminta root; CSP memblokir jaringan). Jalur update Tier 1 adalah B4.
- **Flathub:** sandbox bertentangan dengan pkexec, Waydroid di host, dan
  `/dev/uinput`; hampir semua perintah harus lewat `flatpak-spawn --host`.
- **Migrasi TypeScript penuh dan i18n:** belum ada kebutuhan yang terlihat,
  biayanya tinggi.
- **Multi-instance:** Waydroid hanya punya satu container (BACKEND-TODO §8).
- **Keymapper buatan sendiri** (model mapping, BACKEND-TODO §5): Phantom dan touch
  mode sudah menutup kebutuhan saat ini; tunggu permintaan.
- **Mengganti `CommandResult` dengan enum di wire:** kontraknya stabil dan
  teruji; manfaatnya kecil dibanding perubahan di semua komponen.
- **Memecah `index.css`:** pemindaian 9 Nov tidak menemukan kelas yang tidak
  dipakai (kelas dinamis seperti `toast-${type}` ikut dihitung); belum ada masalah
  nyata yang terselesaikan dengan memecahnya.

---

## 10. Urutan eksekusi

1. **Menuju 0.1.6:** A1, A2, A3, A4, verifikasi A5, B2 langkah (1), B3, C1, F1,
   F2, lalu B1.
2. **Setelah 0.1.6:** C2, C3, F3, D1, D2, E1, B2 langkah (2), B4.
3. **Berikutnya:** E2, E4 (bila ada permintaan), C4, F5, D3.
4. **Saat menyentuh area itu:** A6, D4, D5, E3, E5, F4.

## 11. Aturan untuk setiap item

- Perbaikan bug dimulai dengan test yang gagal, seperti pola P0-4.3 di `RELEASE-GATE-0.1.md`.
- Satu item, satu commit atau lebih dengan Conventional Commits, seperti riwayat
  repo ini.
- Perubahan yang terlihat pengguna masuk `CHANGELOG.md` di `[Unreleased]`; klaim
  README hanya diubah bersama bukti uji.
- Command IPC baru masuk build produksi hanya bersama UI dan test-nya; selain itu
  tetap di balik `dev-commands`.
- Setelah item selesai, centang di sini dengan tanggal dan bukti singkat, seperti
  di `RELEASE-GATE-0.1.md`.
