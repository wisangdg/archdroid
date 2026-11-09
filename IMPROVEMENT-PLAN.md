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

Branch `improve/plan-0.1.6` (dari `feat/touch-mode`), PR #2 ke `main`, belum
di-merge. Setiap item satu commit atau lebih. Direview 9 Nov 2026; temuannya
menjadi R1–R10 di §12.

**Hasil uji saat review:** CI di GitHub hijau, termasuk smoke biner asli, dan
workflow paket hijau (AppImage, `.deb`, `.rpm`, paket Arch). Lokal:
`cargo test` 179 lulus (198 dengan `dev-commands`; 4 test live `#[ignore]`),
`node --test` 51 lulus dan 1 skip, Vitest 80, E2E 7/7; clippy, fmt, ESLint, dan
Prettier bersih. `cargo audit` di CI menangkap 4 advisory pada `zip` 4.6.1, jadi
crate itu diganti `zip` 9 (Rust minimum 1.88).

### Selesai

| Item | Hasil |
|---|---|
| A1, A2 | ✅ Dengan test yang gagal sebelum perbaikan |
| A3 | ✅ Toggle di-disable saat engine mati; properti dibaca ulang saat sesi `RUNNING`, dengan test. Celah: pembacaan ulang itu terjadi sebelum Android selesai boot (R2), dan preset Display belum ikut di-disable (R6) |
| A4 | ✅ Root hanya saat `PermissionDenied` (unit test pemilihan jalur). Di host ini log ber-mode `0666`, jadi View Logs memakai jalur user |
| A5 | ✅ Terbukti lebih parah dari dugaan: `ro.sf.lcd_density` di-set saat boot dan `prop set` ditolak, jadi preset DPI **selalu** gagal. Diganti `wm density` (diuji live: langsung berlaku, bertahan setelah restart). Resolusi (`persist.waydroid.width/height`) diuji live dan bekerja |
| A6 | ✅ Tahap init digabung; status awal `null` |
| B2 | ✅ (1) `.deb`/`.rpm` dicek terhadap glibc 2.35. (2) Workflow membangun AppImage, `.deb`, `.rpm`, dan paket Arch (job container); hijau di GitHub. Tool AppImage (AppRun, linuxdeploy) di-prefetch lewat API dengan checksum, karena URL rilis Tauri menjawab 504 pada 9 Nov |
| B3 | ✅ Termasuk `Cargo.lock` |
| B4 (paket) | ✅ `packaging/aur/PKGBUILD` diuji dengan `makepkg` terhadap v0.1.5 (signature dan checksum lolos); `.SRCINFO` sinkron |
| C1 | ✅ `npm audit` penuh kini 0; risiko dev-only ditutup |
| C2 | ✅ CI ±9–11 menit (`main`, 3 Nov) → ±3,5–4 menit di PR #2 dengan cache hangat (7,7 menit saat cache dingin), sudah termasuk smoke biner asli |
| C3 | ✅ `.github/dependabot.yml` (npm, cargo, actions; dikelompokkan). Baru berjalan setelah merge, karena Dependabot membaca branch default |
| C4 | ✅ `scripts/e2e/real-binary-smoke.py` lulus di PR #2: 9/9 cek (jendela, IPC, CSP memblokir eval, inline script, fetch dan script remote). Celah: cek IPC bisa lolos walau `invoke` gagal (R4) |
| D1 | ✅ `src/lib/tasks.js`; `App.jsx` 807 → 628 baris, sudah termasuk Save APK dan status Booting |
| D2 | ✅ Versi ringan: parser baris di frontend (`src/lib/waydroidStatus.js`) alih-alih mengubah format wire, supaya mock test dan E2E tidak berubah; tidak ada lagi `includes("RUNNING")` di `src/` |
| D3 | ✅ |
| D4 | ✅ Dua langkah: `logs.rs` (saat A4) dan `apk.rs` (saat E4); `lib.rs` 2052 → 1875 baris. Area lain menunggu disentuh |
| D5 | ✅ `HostFacts::probe()` + `report()` murni; 4 test tabel, setiap fakta yang gagal punya baris; laporan host identik sebelum/sesudah |
| E2 | ✅ Command `waydroid_boot_state`, "Android Booting…", test transisi. Celah: double-click dan menu konteks masih bisa Launch saat booting (R3) |
| E4 | ✅ `.apks`/`.xapk`/`.apkm` lewat `adb install-multiple` (`src-tauri/src/apk.rs`, crate `zip` 9 deflate-only). Diuji live: ARCore (4 split) terpasang ulang dari `.apks` dan `.xapk` bersarang; bundle tanpa base ditolak dengan alasan dari Android; OBB ditolak dengan petunjuk. DoD belum penuh: batas jumlah dan ukuran belum diuji (R7), `.apks` buatan bundletool belum dicoba (R8) |
| E5 | ✅ Diputuskan 9 Nov: hanya opsi 1. "Save APK" tanpa root, diuji live (simpan ARCore → pasang ulang). Spike dan opsi data: [`docs/BACKUP-DESIGN.md`](docs/BACKUP-DESIGN.md) |
| F1–F4 | ✅ |
| F5 (sebagian) | ✅ Hardware ini diukur: jendela tampil dalam 212–402 ms; PSS ±290 MB (budget baru: PSS < 350 MB) |

### To-do

Urut prioritas. ⬜ belum · ⏳ menunggu tindakan atau keputusan maintainer ·
❌ diputuskan tidak dikerjakan.
Detail R1–R10 di §12. Item yang selesai diberi ✅ dan tanggal.

| # | Pekerjaan | Prio | Status |
|---|---|---|---|
| R1 | `.pyc` ikut ter-commit; komentar basi di `apk.rs` | P1 | ✅ 9 Nov |
| R2 | Settings dibaca ulang setelah Android selesai boot, bukan saat sesi `RUNNING` | P1 | ✅ 9 Nov (`a8a1c73`) |
| R3 | Launch saat booting juga dicegah di double-click dan menu konteks | P2 | ✅ 9 Nov (`a8a1c73`) |
| R4 | Smoke biner asli memeriksa jawaban IPC secara langsung | P2 | ✅ 9 Nov (`77ea431`) |
| R5 | Paket Arch, `.deb`, dan `.rpm` merekomendasikan adb | P2 | ✅ 9 Nov (`0449a4d`) |
| R6 | Preset Display di-disable saat engine mati atau booting | P2 | ✅ 9 Nov (`a8a1c73`) |
| B1 | Rilis 0.1.6: merge PR #2, smoke rilis, tanda tangan, publikasi (R2–R6 sudah selesai) | P1 | ⏳ maintainer |
| — | Setelah merge: PR Dependabot pertama lewat CI (DoD C3); smoke biner asli hijau di `main` (DoD C4) | P2 | ⏳ setelah merge |
| B4 | Publikasi `archdroid-bin` ke AUR, setelah B1 | P2 | ⏳ maintainer |
| E1 | Kirim patch `fake_touch` ke upstream | P2 | ✅ 9 Nov: [PR #60](https://github.com/waydroid/android_vendor_waydroid/pull/60) (memperbaiki issue #30 yang sudah ada). Sisa: tanggapi review; build image hanya bila diminta |
| F5 | Uji di GNOME, KDE, dan GPU AMD | P2 | ⏳ butuh mesin lain |
| R7 | E4: test batas jumlah split dan ukuran | P3 | ⬜ |
| R8 | E4: uji `.apks` buatan bundletool | P3 | ⬜ |
| R9 | E4: ekstraksi split ke disk, bukan `/tmp` (tmpfs) | P3 | ⬜ |
| R10 | Save APK: tanpa file setengah jadi; tombol sibuk selama menyalin | P3 | ⬜ |
| E3 | Touch mode multi-monitor (catatan di bawah) | P3 | ⏳ uji bersama pengguna |
| E5 opsi 2/3 | Backup data app (butuh root) | — | ❌ Diputuskan 9 Nov: tidak dikerjakan; cukup opsi 1 (Save APK) |
| — | Optimasi daftar app besar dan virtualisasi log terminal (dari arsip gate) | P3 | hanya bila ada laporan lambat |

**Catatan E3 (9 Nov):** dicoba dengan harness aman (jendela fullscreen milik
harness menutup kedua monitor sebelum satu sentuhan pun dikirim). Hyprland 0.56
mengenali touchscreen virtual (`archdroid-touch-probe`, juga `ArchDroid Touch`
dari kode touch mode), tetapi jendela GTK3 maupun GTK4 harness tidak menerima
event sentuh apa pun, jadi pemetaan monitor belum bisa diamati. Langkah
berikutnya: harness dengan klien Wayland yang terbukti menerima `wl_touch`
(mis. `wev`), atau uji langsung dengan Waydroid di DP-2 bersama pengguna. Kode
belum diubah.

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

**Per 9 Nov:** semua item kode di urutan 1–4 sudah dikerjakan, kecuali
yang tercatat di To-do. Sisa (R2–R6 selesai 9 Nov): B1 (merge PR #2 dan rilis) → B4
dan verifikasi C3/C4 setelah merge → R7–R10, E3, F5. E1 terkirim (PR #60);
E5 cukup opsi 1.

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

---

## 12. R — Temuan review 9 Nov 2026

Review atas seluruh diff PR #2 (`d66aa50..ee3480d`): backend Rust, frontend, CI,
packaging, dan dokumen. Semua check lokal dijalankan ulang, dan pembacaan
Settings selama boot diukur live.

Diperiksa dan tidak bermasalah: nama paket divalidasi sebelum `adb shell` dan
sebelum menjadi nama file (`pull_apks`); DPI dibatasi 120–640 sebelum adb; nama
entri bundle direduksi ke nama file, ditulis dengan `create_new` di direktori
`0700`, dan dipotong ke ukuran yang dideklarasikan; log Waydroid dibaca dengan
path yang di-quote; refactor `diagnostics.rs` setara dengan aslinya
(`has_program` sama dengan `resolve_program || trusted_program`); semua action
di-pin ke SHA; `.SRCINFO` sinkron dengan PKGBUILD AUR.

### R1 — Kebersihan repo (P1, S) ✅ 9 Nov
- **Bukti:** `scripts/e2e/__pycache__/real-binary-smoke.cpython-314.pyc` ikut
  ter-commit di `84ef69f`; `.gitignore` tidak mengabaikan `__pycache__/`.
  Komentar `APK_SCAN_BYTES` di `apk.rs` masih menulis "without adding a zip
  dependency", padahal crate `zip` kini dipakai.
- **Dikerjakan:** file di-untrack, `__pycache__/` di-ignore, komentar dibetulkan.

### R2 — Settings dibaca sebelum Android selesai boot (P1, S) ✅ 9 Nov
- **Dikerjakan:** `loadConfigs` jalan saat `status === "online" && !booting`, bukan di `onOnline`. Test Vitest gagal pada kode lama, lulus sekarang. Uji live Start dari keadaan mati: belum.
- **Bukti:** `onOnline` memanggil `loadConfigs()` begitu `waydroid status`
  melaporkan `RUNNING` (`src/App.jsx:43-49`). Diukur live 9 Nov (Waydroid 1.6.3,
  host ini): `waydroid prop get` menunggu layanan platform ±6 s, adb
  `wm density` baru menjawab 7–12 s setelah `RUNNING`, dan
  `sys.boot_completed=1` di ±14 s. `loadConfigs` sampai ke `waydroid_density_get`
  di detik ±7 (`src/hooks/useDeviceConfig.js:58`), dan tidak ada pembacaan
  ulang setelah boot.
- **Dampak:** setelah Start dari keadaan mati, kolom DPI bisa kosong dan preset
  tidak cocok sampai pengguna menerapkan sesuatu. Di host ini hasilnya mepet;
  di host yang lebih lambat, atau pada boot pertama setelah init, hampir pasti
  gagal.
- **Kerjakan:** baca ulang saat `booting` berubah ke `false` selama status
  `online` (efek kecil di `App.jsx`, atau callback `onBooted` dari
  `useEngineStatus`). Kalimat CHANGELOG "when the session comes online" ikut
  disesuaikan.
- **DoD:** test Vitest: online + booting → `waydroid_density_get` dipanggil
  setelah `waydroid_boot_state` menjawab `booted`; uji live: Start dari keadaan
  mati → Settings menampilkan DPI tanpa aksi pengguna.

### R3 — Launch saat booting lewat double-click dan menu konteks (P2, S) ✅ 9 Nov
- **Dikerjakan:** guard di `handleLaunchApp` (toast "Android Is Booting"); test double-click dan menu konteks.
- **Bukti:** hanya tombol Launch di detail app yang di-disable saat booting
  (`AppDetail.jsx`). `AppRow` (double-click) dan `ContextMenu` ("Launch App")
  hanya memeriksa `busy` (`src/components/apps/MyAppsView.jsx:218`, `:248`),
  padahal CHANGELOG menulis "Launch waits for it".
- **Dampak:** launch saat boot tetap bisa lambat atau gagal tanpa penjelasan,
  justru hal yang ingin dicegah E2.
- **Kerjakan:** satu guard di `handleLaunchApp` (`src/App.jsx:182`): saat
  booting, beri toast "Android is still booting" tanpa memanggil backend.
- **DoD:** test: saat booting, double-click dan menu konteks tidak memanggil
  `waydroid_launch_app`.

### R4 — Smoke biner asli bisa lolos walau IPC gagal (P2, S) ✅ 9 Nov
- **Dikerjakan:** smoke memanggil `waydroid_status` langsung lewat `__TAURI_INTERNALS__.invoke` dan mengharapkan `CommandResult`; uji negatifnya ikut di script (command yang tidak ada harus ditolak), jadi tidak perlu build terpisah.
- **Bukti:** bila `invoke("waydroid_status")` melempar error, `checkStatus`
  men-set `isInitialized` ke `true` (`src/hooks/useEngineStatus.js:92-97`). Jadi
  placeholder hilang dan smoke mengambil cabang main UI, yang hanya membuka tab
  tanpa memeriksa jawaban backend (`scripts/e2e/real-binary-smoke.py:93`). Cek
  IPC hanya bermakna bila jalur setup yang terambil (CI tanpa Waydroid).
- **Dampak:** regresi yang membuat `invoke` gagal (mis. command hilang dari
  `generate_handler!`, atau ACL Tauri) tidak memecah job pada jalur itu.
- **Kerjakan:** cek langsung
  `window.__TAURI_INTERNALS__.invoke("waydroid_status")` dan harapkan objek
  dengan `status` berupa string.
- **DoD:** uji negatif sekali secara lokal: tanpa `waydroid_status` di handler,
  smoke gagal.

### R5 — Paket belum merekomendasikan adb (P2, S) ✅ 9 Nov
- **Dikerjakan:** `adb` di `recommends` `.deb`, `android-tools` di `.rpm` dan `optdepends` Arch; dijaga `package-metadata.test.js`.
- **Bukti:** adb kini dibutuhkan preset DPI (A5), split APK (E4), Save APK
  (E5), status Booting (E2), dan Force stop. PKGBUILD AUR mencantumkan
  `android-tools` di `optdepends`, tetapi `packaging/arch/PKGBUILD:15-19`
  (paket Tier 1) tidak, dan `recommends` `.deb`/`.rpm` di
  `src-tauri/tauri.conf.json:51`, `:64` hanya berisi `waydroid`,
  `squashfs-tools`, `lzip`, `git`.
- **Kerjakan:** `android-tools` di `optdepends` Arch; `adb` di `recommends`
  `.deb` (Debian/Ubuntu); `android-tools` di `recommends` `.rpm` (Fedora).
- **DoD:** test metadata (mis. di `package-metadata.test.js`) bahwa ketiga paket
  menyebut adb.

### R6 — Preset Display aktif saat engine mati (P2, S) ✅ 9 Nov
- **Dikerjakan:** Apply dan kedua toggle di-disable sampai Android selesai boot, dengan catatan; test komponen.
- **Bukti:** tombol Apply di
  `src/components/settings/ResolutionPresets.jsx:357-367` hanya di-disable oleh
  `busy`, `isApplying`, `isUnchanged`, dan kolom kosong, bukan oleh status
  engine; A3 hanya menutup kedua toggle. `waydroid prop set` butuh sesi, dan
  `wm density` butuh adb serta Android yang sudah boot.
- **Dampak:** Apply saat engine mati selalu berakhir "Display Config Failed".
- **Kerjakan:** disable Apply saat engine mati atau booting, dengan catatan yang
  sama seperti di toggle.
- **DoD:** test komponen: offline atau booting → Apply disabled; online → aktif.

### R7 — E4: batas split dan ukuran belum diuji (P3, S)
- **Bukti:** DoD E4 meminta test batas ukuran, tetapi `MAX_SPLITS` (64) dan
  `MAX_BUNDLE_BYTES` (4 GiB) di `src-tauri/src/apk.rs:16`, `:18` tidak punya
  test.
- **Kerjakan:** batas sebagai parameter fungsi dalam, lalu test dengan batas
  kecil.
- **DoD:** split ke-65 dan total di atas batas ditolak sebelum ada yang ditulis.

### R8 — E4: `.apks` buatan bundletool belum diuji (P3, M)
- **Bukti:** uji live hanya memakai bundle dari split ARCore (gaya SAI/XAPK).
  `bundletool build-apks` menghasilkan `toc.pb` dan `splits/*.apk` untuk semua
  ABI, density, dan bahasa, plus `standalones/*.apk` bila minSdk < 21.
  `extract_splits` memasang setiap `.apk`, jadi APK standalone ikut dikirim ke
  `install-multiple`.
- **Dampak:** belum diketahui; Android kemungkinan menolak dua APK base dalam
  satu sesi install.
- **Kerjakan:** uji live dengan `.apks` dari bundletool; bila gagal, abaikan
  `standalones/`, dan pertimbangkan memilih split ABI/density yang cocok.
- **DoD:** hasil uji dicatat; unit test untuk struktur bundletool.

### R9 — E4: split diekstrak ke `/tmp` (P3, S)
- **Bukti:** `PrivateDir` memakai `std::env::temp_dir()`
  (`src-tauri/src/apk.rs:198`). Di host ini `/tmp` adalah tmpfs 7,7 GB (RAM),
  sementara batas bundle 4 GiB.
- **Dampak:** bundle game besar memakan RAM selama install, dan bisa gagal
  `ENOSPC` di mesin dengan RAM kecil.
- **Kerjakan:** direktori kerja di `$XDG_CACHE_HOME/archdroid` (disk), tetap
  `0700` dan dihapus setelahnya.
- **DoD:** test lokasi direktori kerja; pesan jelas saat ruang habis.

### R10 — Save APK: file setengah jadi, tanpa status sibuk (P3, S)
- **Bukti:** `write_bundle` (`src-tauri/src/apk.rs:163`) menulis langsung ke
  `~/Downloads/<paket>-<waktu>.apks`. Bila penulisan gagal (mis. disk penuh),
  file terpotong tertinggal dengan nama seperti backup yang sah. Tombol Save APK
  tidak di-disable selama penyalinan dan hanya ada toast info 2,5 detik
  (`src/App.jsx:236`), padahal `adb pull` game besar bisa makan puluhan detik.
- **Kerjakan:** tulis ke `.part`, lalu `rename`; hapus bila gagal. Tandai sibuk
  di UI selama backup.
- **DoD:** test: penulisan yang gagal tidak meninggalkan file; tombol disabled
  selama backup.
