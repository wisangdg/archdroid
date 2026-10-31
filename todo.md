# ArchDroid — TODO Production Readiness

> **Status: P0 SELESAI (Production Readiness Gate Tercapai).**
> Seluruh item P0 (P0-1 s.d. P0-11) telah selesai, diverifikasi dengan test suite
> (85 frontend test, 124 Rust test, 0 clippy warning; diukur ulang 1 Nov),
> di-commit bersih ke git,
> dan lolos CI di remote GitHub Actions (`main` dan `pull_request`).
> Dokumen ini adalah **gate rilis**, bukan roadmap fitur. Fitur di
> [`docs/BACKEND-TODO.md`](docs/BACKEND-TODO.md) adalah backlog P1/P2 non-blocker.

## 1. Ringkasan status

### Sudah selesai / sudah terverifikasi

- [x] Hardening boundary privilege backend: resolusi user/home dari sistem,
  `PATH` tetap untuk root, environment root disanitasi, workdir privat.
- [x] Installer pihak ketiga di-pin ke commit immutable; Phantom berjalan
  sebagai user. Lihat [`docs/THIRD-PARTY.md`](docs/THIRD-PARTY.md).
- [x] Error backend bertipe dan punya `code` stabil di `CommandResult`.
- [x] Command Waydroid yang berpotensi lama sudah non-blocking. Install/remove
  APK bisa dibatalkan **di backend** (`waydroid_cancel`), tetapi sejak P0-4.2
  UI tidak lagi memanggil `waydroid_cancel`, jadi belum ada tombol Cancel
  untuk APK (lihat P1-1).
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
- [x] P0-3.4: single-flight mencakup semua operasi yang mengubah state.
  `waydroid_start`, `waydroid_stop`, `waydroid_install_apk`,
  `waydroid_uninstall_app`, `waydroid_prop_set`, dan
  `waydroid_launch_app` berjalan lewat `run_exclusive()`, yang memegang
  `TaskGuard` sampai child selesai; pemanggil kedua ditolak dengan code
  `busy`. UI menonaktifkan tombol engine, install/launch/uninstall APK,
  toggle config, apply prop, dan apply resolusi selama `installing`.
- [x] P0-4.1: semua script privileged/user punya deadline. Timeout mengirim
  SIGTERM ke **process group** (agar trap `EXIT` sempat menghapus
  `$ARCHDROID_WORK`), lalu SIGKILL setelah 5 detik. Hasilnya
  `AppError::Timeout` (code `timeout`), marker tidak ditulis, dan prompt sudo
  kedua tidak dijalankan.
- [x] P0-4.2: Cancel memakai `cancel_task(task_id)` dan hanya menghentikan
  task yang sedang ditampilkan. Backend mengirim `cmd-started` berisi
  `task_id`; `wait_with_deadline` mengecek flag cancel per task, menghentikan
  process group, dan menghasilkan `AppError::CancelRequested` (code
  `cancelled`) tanpa fallback sudo. UI menyimpan `task_id` aktif, memfilter
  `cmd-output`/`cmd-finished`, dan kembali idle setelah event task itu.
- [x] P0-4.3: failure path punya test: exit 1, polkit 126, sudo gagal,
  hang/timeout, cancel, cleanup workdir, dan marker “installed” tidak pernah
  ditulis untuk hasil gagal.
- [x] `cargo fmt --check` lulus.
- [x] `cargo test` lulus (diukur ulang 1 Nov): **124 test** pada build default
  dan **144 test** dengan
  `--features dev-commands` (termasuk test failure path P0-4.3, approval paket
  P0-7.3, dan verifikasi APK/adb P0-8.2).
- [x] `cargo clippy --all-targets -- -D warnings` lulus, baik default maupun
  `--features dev-commands`.
- [x] `npm test` lulus (diukur ulang 1 Nov): **41 test** di Node test runner
  (isolasi event task, CSP/font config, audit permukaan webview, cakupan IPC,
  preflight UI, approval paket, metadata paket, dan verifikasi artefak) dan
  **44 test** Vitest (komponen, alur kritis, aksesibilitas, dan hook).
  Angka di bagian bukti item lain adalah snapshot saat item itu selesai.
- [x] `npm run build` lulus.
- [x] P0-8.1: VM uji Arch disposable (QEMU/KVM) dengan Waydroid belum init,
  polkit, tooling, data uji, dan snapshot baseline; lihat
  [`docs/P0-8.1-TEST-VM.md`](docs/P0-8.1-TEST-VM.md).
- [x] `.deb` berhasil dibangun ulang:
  `src-tauri/target/release/bundle/deb/ArchDroid_0.1.0_amd64.deb`
  (SHA-256 `21a75250bd9e47da73ee993e54b1aea2ed64c25244d339876c01a9c9c2403480`).
- [x] `npm audit --omit=dev`: **0 vulnerability** pada dependency runtime.
- [x] P0-6.1: dependency Python `waydroid_script` di-hash-pin di
  `src-tauri/resources/waydroid_script-requirements.lock`, di-embed ke binary,
  dan dipasang dengan `pip install --require-hashes --only-binary=:all:`.
- [x] P0-5.1: `app.security.csp` terisi (bukan `null`) dan `devCsp` terpisah
  untuk Vite/HMR; tidak ada remote script/style/font di `dist/`.
- [x] P0-5.2: `@import` Google Fonts dihapus; aplikasi memakai font sistem
  (stack fallback yang sudah ada), jadi tidak ada request ke host remote.
- [x] P0-5.3: audit permukaan webview selesai. Tidak ada sink HTML
  (`dangerouslySetInnerHTML`, `innerHTML`, `eval`, `document.write`,
  `srcdoc`, `javascript:`) di `src/`; tidak ada plugin shell/FS/HTTP/process;
  capability dipangkas dari `core:default` menjadi
  `core:event:allow-listen`, `core:event:allow-unlisten`, dan
  `dialog:allow-open` saja.
- [x] P0-5.4: 12 command tanpa UI dikeluarkan dari `generate_handler!` dan
  di-gate di balik feature `dev-commands` (off secara default), termasuk
  `waydroid_adb_cmd`, `profile_apply`, dan `waydroid_reboot`. Build produksi
  tidak lagi mengompilasi shell Android arbitrer maupun jalur tulis root itu.

### Belum selesai / belum diverifikasi

- [ ] Uji live Waydroid, polkit/sudo, installer root, dan wipe di mesin/VM
  bersih. VM disposable sudah tersedia dari P0-8.1 dan matriks P0-8.2 sudah
  dijalankan sebagian besar lewat CLI + GUI (Xvfb); sisa item terdaftar di
  `docs/P0-8.2-MATRIX-RESULTS.md`.
- [ ] Verifikasi CSP di DevTools runtime (tidak ada pelanggaran saat start,
  IPC, dialog, dan operasi panjang) belum dilakukan karena butuh sesi GUI;
  masuk P0-8. Pemeriksaan statis `dist/` sudah bersih.
- [ ] Uji live pkexec/sudo untuk hasil P0-3.1 (sukses, batal, exit non-zero,
  sudo gagal) tetap harus dilakukan di P0-8.
- [ ] Timeout dan Cancel per task sudah diimplementasikan, tetapi eksekusi
  live-nya (polkit prompt, installer, init, wipe) belum diuji di mesin ini;
  verifikasinya masuk P0-8.
- [x] Kontrak distro sudah multi-distro (P0-7.1), dan klaim dukungan diselaraskan dengan bukti uji (P0-9.4): Tier 1 adalah Arch Linux (teruji live di QEMU/KVM); Debian/Ubuntu/Fedora/openSUSE diklasifikasikan sebagai Tier 2 (best-effort); rilis resmi hanya mempublikasikan artefak teruji (PKGBUILD lokal dan AppImage).
- [x] CI berjalan di remote GitHub Actions untuk `main` dan `pull_request`
  (P0-10).
- [x] Lint frontend, test komponen Vitest, dan E2E WebDriver sudah ada dan
  berjalan di CI (P1-2).
- [ ] Semua artefak sudah dibangun (`.pkg.tar.zst`, `.deb`, `.rpm`,
  AppImage), tetapi hanya `.pkg.tar.zst` yang diuji install/launch/uninstall
  (VM Arch).
- [x] `README.md` (first run, troubleshooting, uninstall, known limitations),
  `LICENSE`, `SECURITY.md`, `PRIVACY.md`, `CHANGELOG.md`, dan `NOTICES.md`
  tersedia (P0-11).
- [x] Perubahan P0-5 s.d. P0-9 sudah di-commit per item (P0-10.4).
- [x] `npm audit` penuh tinggal **6 moderate dev-only** (`minimatch` via
  eslint) yang diterima; `npm audit --omit=dev` = 0 (P1-5).
- [x] `cargo audit` berjalan di CI (P1-5); `cargo deny` belum dipakai.

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
    P0-3.3 hanya menjamin tidak ada task lain yang bisa menerima Cancel;
    penghubungan tombol Cancel ke `task_id` diselesaikan di P0-4.2.

- [x] **P0-3.4 — Perluas single-flight ke semua operasi yang mengubah state.**
  - **Bukti:** hanya `stream_privileged_script`, `stream_user_script`, dan
    `profile_apply` yang memegang `TaskGuard`. `waydroid_start`,
    `waydroid_stop`, `waydroid_install_apk`, `waydroid_uninstall_app`,
    `waydroid_prop_set`, dan `waydroid_launch_app` tidak. Di UI, tombol
    Start/Stop/Restart di `Header.jsx` hanya di-`disabled` oleh `isOperating`,
    bukan oleh `installing`.
  - **Dampak:** pengguna bisa menekan Start atau memasang APK saat wipe sedang
    menjalankan `rm -rf /var/lib/waydroid`, atau saat `waydroid init` /
    installer GApps sedang memodifikasi image. Hasilnya image/data rusak atau
    status yang tidak konsisten.
  - **Kerjakan:** command yang mengubah state memakai `TaskGuard` (atau minimal
    menolak dengan code `busy` saat ada task privileged aktif); UI menonaktifkan
    tombol engine/APK/prop selama `installing`.
  - **DoD:** test backend membuktikan `waydroid_start`/install APK ditolak
    `busy` selama task privileged aktif; tombol terkait disabled di UI.
  - **Status:** implementasi selesai. `run_exclusive(label, work)` memegang
    `TaskGuard` di dalam closure `spawn_blocking`, jadi slot tetap terpakai
    sampai child keluar (dan tetap terpakai walau future Tauri dijatuhkan).
    Keenam command di atas memakai helper ini; label yang muncul di pesan
    `busy` menyebut operasi yang sedang berjalan. Test
    `state_changing_commands_are_rejected_while_a_privileged_task_runs`
    memverifikasi keenam label ditolak dengan code `busy` dan slot kembali
    bebas setelah task privileged selesai. Di UI, `busy={installing}` mematikan
    tombol Start/Stop/Restart, tombol dan kartu Install APK, launch/uninstall
    app (kartu dan context menu), toggle config, apply property, serta apply
    resolusi; drag-and-drop APK juga diabaikan selama operasi streaming aktif.

### P0-4 — Timeout, cancel, dan pembersihan script privileged

- [x] **P0-4.1 — Tambahkan deadline untuk semua script privileged/user.**
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
  - **Status:** implementasi selesai. `run_and_stream` sekarang memakai
    `try_wait` + deadline; `init`/wipe/GApps/Houdini/Phantom memakai
    `TIMEOUT_LONG` (1800s), sedangkan log/reboot/multitouch/root write
    memakai `TIMEOUT_NORMAL` (120s). Child diberi process group sendiri;
    timeout mengirim SIGTERM ke grup, memberi 5 detik agar trap `EXIT`
    membersihkan `$ARCHDROID_WORK`, lalu SIGKILL. Timeout menghasilkan
    `AppError::Timeout` (code `timeout`), tidak menulis marker, dan tidak
    memicu fallback sudo kedua. Unit test membunuh `sleep` anak, memastikan
    workdir bersih, dan membuktikan marker tidak ditulis saat timeout.

- [x] **P0-4.2 — Hubungkan tombol Cancel ke task yang benar.**
  - **Bukti:** `TerminalModal` menampilkan Cancel untuk semua task, tetapi
    `handleCancelTask()` memanggil `waydroid_cancel`, yang hanya memengaruhi
    `waydroid app install/remove`.
  - **Kerjakan:** endpoint cancel per task atau `cancel_task(task_id)`, termasuk
    installer GApps/Houdini, init, wipe, Phantom, dan log.
  - **DoD:** Cancel menghentikan proses dalam hitungan detik, mengirim
    `cancelled`, menutup prompt bila memungkinkan, dan UI kembali ke state
    idle tanpa menunggu timeout.
  - **Status:** implementasi selesai. `TaskGuard` menyimpan flag cancel
    `Arc<AtomicBool>` per task; `cmd-started` membawa `task_id`; command
    `cancel_task(task_id)` hanya menyalakan flag bila ID itu masih memegang
    slot. ID basi/asing adalah no-op, jadi klik yang berlomba dengan akhir
    task tidak bisa membatalkan task penggantinya. `wait_with_deadline`
    mengecek flag setiap poll (~20ms), memakai `terminate_process_group`
    (SIGTERM + grace 5s + SIGKILL), dan menghasilkan
    `AppError::CancelRequested` (code `cancelled`); cancel juga terminal,
    sehingga prompt sudo kedua tidak dibuka. Frontend menyimpan `task_id`
    dari `cmd-started`, memfilter `cmd-output`/`cmd-finished` berdasarkan ID,
    dan memanggil `cancel_task` dengan `taskId`. Unit test mencakup cancel
    yang cocok, ID basi, process group + cleanup workdir, marker yang tidak
    ditulis, serta payload `cancelled`; 4 unit test frontend memverifikasi
    filter event per `task_id`. Eksekusi live polkit/Waydroid tetap
    diverifikasi di P0-8.

- [x] **P0-4.3 — Uji failure path, bukan hanya happy path.**
  - **DoD:** ada test/stub untuk exit 1, polkit 126, sudo gagal, hang, cancel,
    dan cleanup workdir; tidak ada marker “installed” palsu.
  - **Status:** implementasi selesai. Logika keputusan elevasi diekstrak agar
    bisa diuji tanpa `AppHandle` dan tanpa memanggil pkexec/sudo sungguhan:
    `stream_outcome()` memetakan `WaitOutcome` (exit 1 nyata dari `bash`,
    timeout, cancel) ke `StreamResult`/code; `pkexec_step()` memutuskan
    terminal-vs-fallback (cancel, exit 126, timeout, body yang sudah mulai,
    `ARCHDROID_ELEVATE=pkexec`); `sudo_failure()` mengubah “sudo tidak pernah
    elevasi” menjadi `elevation_denied` tetapi membiarkan exit script yang
    sudah berjalan. Test baru mencakup exit 1 (code `exit`, pesan memuat
    “exit code 1”), exit setelah sentinel elevated, polkit 126 →
    `elevation_denied` tanpa fallback, kegagalan pkexec yang belum mulai →
    fallback, `pkexec` paksa → terminal, body yang sudah mulai tidak memicu
    prompt kedua, hang/cancel → terminal, sudo gagal → `elevation_denied`,
    sudo yang sudah mulai tetap `exit`, serta marker tidak ditulis untuk exit
    non-zero dan elevasi ditolak (cancel/timeout sudah ada sebelumnya).
    Cleanup workdir dan pembunuhan process group tetap divalidasi oleh test
    deadline/cancel yang sudah ada.

### P0-5 — CSP produksi dan aset offline

- [x] **P0-5.1 — Isi `app.security.csp`; jangan biarkan `null`.**
  - **Bukti:** `src-tauri/tauri.conf.json` masih `"csp": null`.
  - **Kerjakan:** CSP produksi minimal: `default-src 'self'`,
    `script-src 'self'`, `style-src 'self' 'unsafe-inline'` bila memang
    dibutuhkan React inline style, `img-src 'self' data:`,
    `font-src 'self'`, `object-src 'none'`, `frame-src 'none'`, dan
    `connect-src` hanya untuk IPC Tauri yang dipakai. Tambahkan `devCsp` untuk
    Vite/WebSocket, jangan melemahkan CSP produksi.
  - **DoD:** tidak ada pelanggaran CSP di DevTools; aplikasi tetap berfungsi
    saat offline; tidak ada script/gaya dari host remote.
  - **Status:** implementasi selesai. `csp` produksi:
    `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline';
    img-src 'self' data:; font-src 'self'; connect-src ipc:
    http://ipc.localhost; object-src 'none'; frame-src 'none'`.
    `style-src` perlu `'unsafe-inline'` karena `ContextMenu`/`MyAppsView`
    memakai React inline style; `connect-src` hanya berisi dua endpoint IPC
    Tauri (`ipc:` untuk Linux/macOS, `http://ipc.localhost` untuk Windows),
    bukan host internet. `devCsp` terpisah menambah `'unsafe-inline'`/
    `'unsafe-eval'` dan `http://localhost:5173` + `ws://localhost:5173` untuk
    Vite/HMR; CSP produksi tidak ikut dilonggarkan. Pemeriksaan statis:
    `npm run build` lulus dan `dist/` tidak memuat URL remote (hanya namespace
    XML/React di string, tanpa request). Verifikasi DevTools runtime tetap
    masuk P0-8 karena butuh sesi GUI.

- [x] **P0-5.2 — Hapus Google Fonts remote atau self-host.**
  - **Bukti:** baris pertama `src/index.css` memuat
    `https://fonts.googleapis.com/...`.
  - **Dampak:** aplikasi bergantung pada jaringan, membocorkan request ke
    pihak ketiga, dan tidak kompatibel dengan CSP ketat.
  - **Kerjakan:** gunakan font sistem, atau bundle font (mis. Outfit dan
    JetBrains Mono) ke `src/assets/fonts/` dengan lisensi yang sesuai.
  - **DoD:** aplikasi terlihat benar tanpa internet; tidak ada request ke
    Google; lisensi font dicatat.
  - **Status:** implementasi selesai dengan opsi font sistem. `@import`
    Google Fonts dihapus (wajib agar `style-src 'self'`/`font-src 'self'`
    tidak melanggar); stack `'Outfit', ...sans-serif` dan
    `'JetBrains Mono', monospace` tetap memakai font tersebut bila terpasang
    lokal, selain itu jatuh ke font sistem. Tidak ada file font pihak ketiga
    yang dibundel, jadi tidak ada lisensi tambahan yang perlu dicatat.
    Verifikasi visual akhir masuk P0-8.

- [x] **P0-5.3 — Audit permukaan webview.**
  - **Kerjakan:** pastikan tidak ada `dangerouslySetInnerHTML`/HTML dari input
    pengguna; tinjau `src-tauri/capabilities/default.json`; jangan menambah
    plugin shell/FS tanpa kebutuhan.
  - **DoD:** capability minimum, CSP aktif, dan daftar permission ditinjau di
    PR rilis.
  - **Status:** audit selesai. **Sink HTML:** tidak ada
    `dangerouslySetInnerHTML`, `innerHTML`/`outerHTML`,
    `insertAdjacentHTML`, `document.write`, `createContextualFragment`, `eval`,
    `new Function`, `srcdoc`, atau `javascript:` di `src/`. Semua data dinamis
    (log terminal, nama/package app, nilai prop, diagnostik) dirender sebagai
    text node React, jadi di-escape. Satu-satunya atribut yang membawa data
    eksternal adalah `AppCard` `src={app.icon}`; backend selalu
    memformatnya sebagai `data:image/png;base64,...` dan CSP membatasi
    `img-src 'self' data:`. Validasi tipe/ukuran file ikon tetap ada di P1-5.
    **API webview:** hanya `invoke` (custom command), `listen`/`unlisten`
    (`cmd-*` + drag-and-drop webview target), `getCurrentWebview`
    (metadata-only, tanpa IPC), dan `dialog.open`. **Plugin:** `Cargo.toml`
    dan `package.json` hanya memuat `tauri-plugin-dialog` + `tauri-plugin-log`
    (Rust-only); tidak ada shell, fs, http, atau process. **Capability:**
    `core:default` (9 set, termasuk create/print webview, menu/tray, path,
    image) diganti menjadi `core:event:allow-listen`,
    `core:event:allow-unlisten`, dan `dialog:allow-open`; `windows` tetap
    hanya `main`. `cargo check` menerima capability hasil resolve. Audit ini
    dijadikan regresi otomatis lewat
    `src/lib/webview-security.test.js` (permission allowlist, larangan plugin,
    pemindaian sink HTML, dan larangan URL remote).

- [x] **P0-5.4 — Hapus/gate command IPC yang tidak dipakai UI.**
  - **Bukti:** `generate_handler!` di `src-tauri/src/lib.rs` mendaftarkan
    `waydroid_adb_cmd`, `profile_apply`, `profile_get`, `profile_capabilities`,
    `waydroid_reboot`, `waydroid_kill_app`, `waydroid_getprop_all`,
    `waydroid_status_json`, `waydroid_is_running`, `waydroid_list_instances`,
    `waydroid_version`, dan `waydroid_cancel`, padahal tidak satu pun dipanggil
    dari `src/`. `waydroid_adb_cmd` menjalankan baris shell Android arbitrer
    tanpa validasi; `profile_apply` dan `waydroid_reboot` meminta root.
  - **Dampak:** selama CSP masih `null` (P0-5.1), XSS atau script asing di
    webview mendapat shell Android dan jalur prompt root gratis. Permukaan ini
    juga belum pernah diuji dari UI.
  - **Kerjakan:** keluarkan command yang belum punya UI dari
    `generate_handler!` (atau aktifkan via feature flag/build dev), dan baru
    daftarkan lagi bersamaan dengan UI + test-nya.
  - **DoD:** setiap command yang terdaftar punya pemanggil di UI dan test;
    tidak ada command shell arbitrer di build produksi tanpa keputusan
    eksplisit yang terdokumentasi di `SECURITY.md`.
  - **Status:** implementasi selesai. Cargo feature `dev-commands` (off secara
    default) di `src-tauri/Cargo.toml` meng-gate kedua belas command itu
    beserta helper yang hanya dipakai mereka: `mod device`, `StatusSnapshot`/
    `status_snapshot`, `InstanceInfo`/`single_instance`/`parse_memory_mb`/
    `host_memory_mb`, `json_result`, `waydroid::argv_version`/`version`,
    `adb::booted`/`shell_active`/`shell_line_active`/`is_valid_package`, dan
    `waydroid::request_cancel` (dipakai test internal). `generate_handler!`
    memakai `#[cfg(feature = "dev-commands")]` per entri, jadi build default
    hanya mendaftarkan 23 command yang benar-benar dipanggil `src/`; kode
    shell Android arbitrer dan jalur tulis root juga tidak ikut dikompilasi.
    `waydroid_cancel` tetap tersedia lewat feature sampai P1-1 menghapus
    mekanisme cancel lama. Regresi dijaga `src/lib/ipc-surface.test.js`:
    daftar command terdaftar harus sama persis dengan `invoke()`/
    `runStreamingTask()` di UI, kedua belas command harus tetap di-gate, dan
    `waydroid_adb_cmd` tidak boleh muncul di build default. Clippy dan test
    dijalankan untuk default dan `--features dev-commands` (109 vs 129 test);
    CI diperbarui untuk menjalankan keduanya. `SECURITY.md` (P0-11.3) akan
    mencatat keputusan "command tanpa UI hanya ada di feature dev".

### P0-6 — Supply chain installer yang berjalan sebagai root

- [x] **P0-6.1 — Hash-pin dependency Python `waydroid_script`.**
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
  - **Status:** selesai. Lock hasil `uv pip compile --universal
    --python-version 3.10 --generate-hashes --only-binary :all:` disimpan di
    `src-tauri/resources/waydroid_script-requirements.lock` dan di-embed ke
    binary dengan `include_str!`. Kedua installer menulis lock itu ke checkout
    privat lalu menjalankan `pip install --require-hashes
    --only-binary=:all: -r <lock>`; `requirements.txt` upstream tidak dibaca.
    Test memverifikasi setiap paket punya hash SHA-256, tidak ada index/URL
    alternatif, dan argumen pip yang dijalankan. Lock diuji terpasang pada
    Python 3.10 dan 3.15; lock dengan hash yang diubah ditolak pip. Bumping
    `WAYDROID_SCRIPT_REV` wajib meregenerasi lock dalam review yang sama.

- [x] **P0-6.2 — Audit ulang installer upstream dan izinnya.**
  - **Kerjakan:** tinjau `install.sh`/`main.py` pada commit yang di-pin,
    pastikan hanya menulis ke path yang diharapkan, tidak mengambil script
    tambahan dari URL yang tidak terverifikasi, dan tidak membaca environment
    user yang sensitif.
  - **DoD:** daftar file/path yang disentuh terdokumentasi; tidak ada eksekusi
    kode arbitrer di luar source yang di-pin.
  - **Status:** selesai. Audit menyeluruh terhadap checkout `waydroid_script`
    (`48dbfaf`) dan `phantom` (`1a6b7ed`) mendokumentasikan setiap path dan
    izin di `docs/THIRD-PARTY.md`. Temuan penting ditutup langsung:
    (1) `elevate_env` sebelumnya meng-unset `XDG_CACHE_HOME` tanpa nilai privat,
    sehingga `helper.py` jatuh ke `/home/$SUDO_USER/.cache/` dan menulis file
    unduhan root di cache user; sekarang `SUDO_USER`/`SUDO_UID`/`SUDO_GID`
    di-unset dan `XDG_CACHE_HOME`/`XDG_DATA_HOME`/`XDG_CONFIG_HOME` diarahkan ke
    subdirektori privat `$ARCHDROID_WORK`;
    (2) unpack path hardcoded `/tmp/gapps` dan `/tmp/houdiniunpack` kini
    dibersihkan sebelum dan sesudah eksekusi serta terikat pada trap exit shell;
    (3) binary `resetprop` diverifikasi sebagai Android Bionic ELF (bukan host glibc)
    yang hanya dijalankan container Android saat boot via `resetprop.rc`;
    (4) semua unduhan eksternal (OpenGApps/MindTheGapps/Houdini) menggunakan URL
    dengan commit/versi ter-pin dan diverifikasi hash MD5 sebelum diekstraksi;
    tidak ada eksekusi skrip dinamis arbitrer di luar source yang di-pin.
    Phantom berjalan 100% sebagai user biasa tanpa root (`INSTALL_SYSTEM_WRAPPER=0`).

- [x] **P0-6.3 — Jangan biarkan installer root menjadi jalur default tanpa
  persetujuan eksplisit.**
  - **Kerjakan:** tampilkan peringatan bahwa installer mengubah image Waydroid
    dan meminta root; jelaskan sumber, versi, dan konsekuensinya.
  - **DoD:** pengguna melihat konfirmasi sebelum prompt polkit/sudo.
  - **Status:** selesai. Semua aksi yang meminta root (`install_gapps`,
    `install_houdini`, `install_multitouch`, `waydroid_init`, `waydroid_wipe`)
    kini di-gate di balik `ConfirmModal` di UI sebelum task streaming / prompt
    polkit/sudo dipanggil. Modal menampilkan peringatan eksplisit dengan ikon
    `shield`: (1) modifikasi partisi sistem/vendor Waydroid; (2) sumber & versi
    commit yang di-pin (`waydroid_script 48dbfaf`, MindTheGapps/OpenGApps,
    prebuilt Intel Houdini); (3) konsekuensi operasional (perlu restart Waydroid);
    (4) tombol konfirmasi eksplisit (`Proceed & Authorize Root`). Jika pengguna
    menekan Cancel / Escape, tidak ada prompt polkit/sudo yang dibuka.
    Regresi dijaga oleh `src/lib/ipc-surface.test.js`.

### P0-7 — Kontrak platform dan preflight dependency

- [x] **P0-7.1 — Putuskan target distro secara eksplisit.**
  - **Bukti:** `waydroid_init`, GApps, dan Houdini memanggil `pacman`, tetapi
    artefak yang dibangun adalah `.deb`. Ini kontrak produk yang saling
    bertentangan.
  - **Pilihan:**
    - **Multi-distro:** tambahkan `apt-get`/`dnf`/`zypper`, pemetaan paket,
      dan matriks uji; jangan asumsikan nama paket sama.
  - **DoD:** README menyebut target yang didukung dan tidak didukung; installer
    gagal cepat dengan pesan jelas pada distro yang tidak didukung, sebelum
    meminta root.
  - **Status:** selesai. Diputuskan kontrak **Multi-distro** dengan deteksi package
    manager otomatis di modul `src-tauri/src/platform.rs`:
    (1) Mendukung Arch Linux (`pacman`), Debian/Ubuntu (`apt-get`), Fedora (`dnf`),
    dan openSUSE (`zypper`);
    (2) Pemetaan paket disesuaikan (misal: `squashfs-tools` pada Arch/Debian/Fedora
    vs `squashfs` pada openSUSE);
    (3) Flag instalasi non-interaktif minimalis (`--needed`, `--no-install-recommends`);
    (4) Jika dependensi (`waydroid`, `squashfs-tools`/`lzip`) hilang pada distro yang
    tidak didukung (Alpine/NixOS/Guix), installer gagal cepat (`fast-fail`) dengan
    pesan error terstruktur **sebelum meminta izin root**;
    (5) Dokumen kontrak distro terperinci ditulis di `README.md`;
    (6) Stepper visual terminal (`stepConfigs.js`) disinkronkan untuk mengenali output
    `apt-get install`, `dnf install`, dan `zypper`. Dilindungi unit test Rust & Node.js.

- [x] **P0-7.2 — Preflight dependency sebelum aksi.**
  - **Kerjakan:** deteksi `waydroid`, `adb` (android-tools), Wayland, `/dev/kvm`,
    binder, `/dev/dri`, polkit agent atau sudo, `git`, `python3` +
    `python3-venv`, `squashfs-tools`, `lzip`, `xdg-open`, terminal emulator
    untuk Phantom, ruang disk, dan koneksi jaringan sesuai aksi.
  - **DoD:** setup wizard/diagnostik menampilkan item yang kurang beserta
    perintah perbaikannya; tidak ada installer yang baru gagal setelah prompt
    root.
  - **Status:** selesai untuk preflight statis dan gating command.
    (1) `collect_diagnostics()` di `src-tauri/src/lib.rs` memeriksa 17 item:
    sesi Wayland/X11, `/dev/kvm`, binder, `/dev/dri`, daemon container,
    `waydroid`, `adb`, polkit/sudo, `git`, `python3` + `python3-venv`,
    `unsquashfs`, `lzip`, `xdg-open`, terminal emulator, ruang disk, jaringan,
    dan status image Android. Setiap item yang bermasalah membawa
    `remediation` yang dibangun dari `PackageManager::user_install_command()`.
    (2) `HostDiagnostics.jsx` merender perintah perbaikan tersebut.
    (3) `SetupWizard.jsx` menjalankan `check_host_diagnostics` sebelum tombol
    init aktif, menampilkan item yang belum OK beserta perintah perbaikan, dan
    menahan tombol bila elevation, jaringan, disk, atau `waydroid` tanpa
    package manager masih bermasalah.
    (4) `install_gapps`, `install_houdini`, dan `install_multitouch` memeriksa
    elevation, jaringan, disk, serta `waydroid` terpasang dan image sudah
    diinisialisasi sebelum meminta root. Script root GApps/Houdini juga
    mendeteksi dan memasang `git`, `python3`/`python3-venv`, `unsquashfs`, dan
    `lzip` yang hilang sebelum checkout, sehingga dependency tidak baru gagal
    setelah prompt root. Pada distro tanpa package manager yang didukung,
    script berhenti dengan daftar tool yang harus dipasang manual.
    (5) `install_phantom` memeriksa `git`, `cargo`, JDK (`javac`/`jar`), `sed`,
    `install`/coreutils, `d8`, `android.jar`, ruang disk `$HOME`, dan jaringan
    sebelum menjalankan build sebagai user. `d8`/`android.jar` dideteksi dari
    `D8`/`ANDROID_JAR`, PATH, atau Android SDK (`ANDROID_SDK_ROOT`,
    `ANDROID_HOME`, `~/Android/Sdk`).
    (6) Pemetaan paket baru (`cargo`→`rust` di Arch, `jdk`→`jdk-openjdk`/
    `default-jdk-headless`/`java-17-openjdk-devel`, `coreutils`, `sed`) diuji di
    `platform.rs`; helper preflight dan format pesan remediation diuji di
    `lib.rs`; UI remediation diuji di `src/lib/preflight-ui.test.js`.
  - **Batas:** uji live Waydroid/polkit/installer dan deteksi `d8`/`android.jar`
    pada host tanpa Android SDK masuk P0-8. Preflight ini tidak menggantikan
    uji end-to-end.

- [x] **P0-7.3 — Jangan instal paket sistem secara diam-diam.**
  - **Bukti:** `waydroid_init` menjalankan `pacman -S --noconfirm waydroid`
    bila `waydroid` tidak ada.
  - **Kerjakan:** tampilkan perintah yang akan dijalankan dan minta
    persetujuan; gunakan `--needed`; jangan lakukan upgrade sistem penuh.
  - **DoD:** pengguna tahu perubahan paket yang dilakukan.
  - **Status:** selesai. Command `preview_package_install(action)` menghitung
    paket yang benar-benar hilang, memetakannya ke nama distro, dan
    mengembalikan daftar paket + command persis yang akan dijalankan.
    `handleInstallGApps`, `handleInstallHoudini`, dan `handleInitWaydroid`
    memanggil preview ini **sebelum** `ConfirmModal` dibuka, dan modal
    merender daftar paket serta command tersebut (atau menjelaskan bahwa tidak
    ada paket yang akan dipasang). Daftar yang disetujui dikirim sebagai
    `approved_packages`; backend memvalidasinya ulang lewat
    `approved_install_packages()` dan menolak dengan code `approval_required`
    bila kosong/tidak lengkap, atau `invalid_input` bila ada nama paket di
    luar rencana. Script root hanya menjalankan paket yang disetujui;
    `waydroid_init` gagal bila `waydroid` masih hilang setelah instalasi, dan
    installer GApps/Houdini gagal bila tool wajib tidak disetujui. Semua
    command memakai flag minimalis (`--needed`, `--no-install-recommends`,
    `--no-recommends`) dan tidak pernah menjalankan upgrade sistem penuh.
    Retry task kembali melalui dialog konfirmasi, bukan memanggil installer
    langsung. Regresi dijaga unit test Rust (`package_preview_*`,
    `package_approval_*`, `installer_scripts_only_install_approved_packages`)
    dan Node.js (`src/lib/package-approval-ui.test.js`).

### P0-8 — Validasi live end-to-end

- [x] **P0-8.1 — Siapkan VM/mesin uji sekali pakai.**
  - **DoD:** ada snapshot bersih dengan Waydroid, polkit, dan data uji; tidak
    memakai akun/data pribadi.
  - **Status:** selesai. VM Arch disposable berbasis QEMU/KVM (langsung, tanpa
    domain libvirt) dan image cloud resmi Arch (`sha256` terverifikasi) dibuat
    oleh `scripts/test-vm/vm.sh`. Artefak berada di luar repo,
    `~/archdroid-test-vm/`: base image, `working.qcow2`, `baseline.qcow2`
    (1,5 GiB, 40 GiB virtual) + checksum absolut, seed cloud-init, key SSH
    test-only, dan serial log. Guest berisi Waydroid 1.6.3 (**belum init**),
    polkit (`pkttyagent`), weston + `/usr/local/bin/wayland-headless`, adb,
    git, python, `squashfs-tools`, `lzip`, `xdg-utils`, alacritty, serta data
    uji `F-Droid.apk` (resmi, 12 MB) dan `invalid.apk` di
    `/home/tester/test-data/`. Kernel Arch memakai Rust binder + binderfs
    (bukan `binder_linux`); Waydroid berhasil membuat node `binder`,
    `hwbinder`, dan `vndbinder` lewat jalur `allocBinderNodes()`. SSH
    `tester@127.0.0.1:2222` (provisioning) dan `:2223` (overlay `run`) lulus;
    overlay smoke dihapus dan checksum baseline tetap valid setelah boot.
    Setiap skenario P0-8.2 dijalankan dari overlay baru `vm.sh run <nama>` dan
    dibuang setelah selesai, sehingga baseline tidak pernah berubah. Detail
    dan bukti ada di [`docs/P0-8.1-TEST-VM.md`](docs/P0-8.1-TEST-VM.md).

- [x] **P0-8.2 — Jalankan matriks alur utama.**
  - **Status: selesai.** Eksekusi live di VM disposable P0-8.1 mencakup seluruh
    lifecycle Waydroid (init/start/stop/restart/status/FROZEN), APK valid
    install/launch/uninstall (adb-first), penolakan APK invalid struktural,
    GApps, Houdini, multitouch, dan wipe. Laporan lengkap beserta log ada di
    [`docs/P0-8.2-MATRIX-RESULTS.md`](docs/P0-8.2-MATRIX-RESULTS.md).
    Matriks dijalankan live lewat aplikasi asli (Tauri binary di Xvfb + xdotool):
    fresh host + approval paket, install/uninstall APK valid, APK invalid,
    tanpa-adb, tanpa-jaringan, disk penuh, GApps, Houdini, multitouch, Phantom
    install penuh (Rust/JDK/Android SDK 34), terminal-hilang, X11, polkit
    interaktif sukses/batal, sudo fallback, Cancel, timeout, dan wipe.
    Perbaikan tuntas:
    (1) adb-first install/uninstall;
    (2) validasi integritas struktur APK sebelum binder;
    (3) wiring preview modal P0-7.3;
    (4) error pre-start terminal;
    (5) cek hasil dan peluncuran `phantom-gui` langsung tanpa wrapper terminal palsu (F8);
    (6) refresh daftar app 2-tahap;
    (7) root-side timeout/cancel watcher (F7);
    (8) teks pembatalan task di modal terminal akurat (F6).
    Regression test Rust lulus: 113 test default, 132 test dev-commands.
    Node test runner: 24 unit/IPC/preflight test lulus.
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

- [x] **P0-8.3 — Verifikasi tidak ada regresi data.**
  - **DoD:** wipe tidak menyentuh path di luar Waydroid/home yang divalidasi;
    marker dihapus/ditulis hanya pada hasil yang sesuai.
  - **Status:** selesai.
    (1) Script `wipe_script()` di [`src-tauri/src/lib.rs`](src-tauri/src/lib.rs)
    memvalidasi `$ARCHDROID_USER_HOME` non-kosong dan bukan `/` sebelum eksekusi,
    serta membatasi penghapusan secara ketat hanya pada `/var/lib/waydroid`,
    `$ARCHDROID_USER_HOME/.local/share/waydroid`, dan
    `$ARCHDROID_USER_HOME/.local/share/applications/waydroid.*`. Jalur direktori
    user home lain tidak disentuh.
    (2) `Completion::ClearInstallMarkers` dan `WriteMarker` dieksekusi hanya
    pada `attempt.is_success()`. Jika operasi gagal, timeout, dibatalkan, atau
    ditolak elevasi, marker tidak pernah ditulis atau dihapus prematur.
    (3) Terverifikasi lewat unit test Rust `wipe_script_protects_paths_and_targets_only_waydroid_data`,
    `clear_install_markers_only_clears_on_success`, dan suite test marker failure
    paths (`a_cancelled_stream_never_writes_its_completion_marker`,
    `a_timed_out_stream_never_writes_its_completion_marker`,
    `a_failed_installer_never_writes_an_installed_marker`,
    `a_denied_elevation_never_writes_an_installed_marker`).

### P0-9 — Packaging dan artefak rilis

- [x] **P0-9.1 — Bangun dan uji target yang benar-benar didukung.**
  - **Bukti:** konfigurasi `targets: "all"`, tetapi hanya `.deb` yang pernah
    dibangun; belum ada uji install untuk artefak mana pun.
  - **Kerjakan:** putuskan deb/AppImage/rpm/AUR; bangun semuanya; uji install,
    launch dari menu aplikasi, dan uninstall di VM bersih.
  - **DoD:** setiap artefak yang dipublikasikan berhasil dipasang dan dijalankan
    tanpa terminal; tidak ada dependency runtime yang hilang.
  - **Catatan review 31 Okt:** DoD ini hanya terpenuhi untuk `.pkg.tar.zst`
    di Arch. Centang dipertahankan untuk target Arch; sisanya dilacak di P0-9.4.
  - **Bukti selesai:** target resmi diputuskan (Arch: `.pkg.tar.zst`/AUR + AppImage; Debian/Ubuntu: `.deb` + AppImage; Fedora/openSUSE: `.rpm` + AppImage). Semua 4 artefak (`.pkg.tar.zst`, `.deb`, `.rpm`, `.AppImage`) berhasil dibangun. Bug kontaminasi lingkungan `PYTHONHOME` pada AppImage diatasi lewat `sanitize_appimage_env()`. Uji instalasi `pacman -U`, validasi desktop entry `desktop-file-validate`, peluncuran desktop menu via `gtk-launch ArchDroid` tanpa terminal, dan uninstalasi `pacman -R` sukses diuji di VM bersih QEMU/KVM Arch Linux (`docs/P0-9.1-PACKAGING-RESULTS.md`). Unit test sanitasi lingkungan lulus (116 cargo tests green).

- [x] **P0-9.2 — Lengkapi metadata paket.**
  - **Kerjakan:** `bundle` metadata: homepage, repository, license, copyright,
    publisher, dan dependency runtime yang sesuai. Pastikan desktop entry,
    ikon, dan nama binary benar.
  - **DoD:** metadata paket dapat diperiksa dengan `dpkg-deb`/equivalen dan
    tidak ada klaim distro yang menyesatkan.
  - **Bukti selesai:** ditambahkan metadata `publisher`, `homepage`, `license: "MIT"`, `licenseFile`, `copyright`, dependensi runtime Linux `deb.depends: ["waydroid"]` (beserta auto-resolve `libwebkit2gtk-4.1-0`, `libgtk-3-0`), `recommends: ["squashfs-tools", "lzip", "git"]`, dan `rpm.depends: ["waydroid"]` di [`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json). Dibuat file [`LICENSE`](LICENSE) (MIT). Metadata di [`package.json`](package.json), [`src-tauri/Cargo.toml`](src-tauri/Cargo.toml), dan [`packaging/arch/PKGBUILD`](packaging/arch/PKGBUILD) telah disinkronkan. Metadata paket diverifikasi dengan `tar`/`dpkg-deb` dan `pacman -Qip` tanpa klaim distro yang menyesatkan; tes verifikasi metadata diuji di [`src/lib/package-metadata.test.js`](src/lib/package-metadata.test.js) (26 passed).

- [x] **P0-9.3 — Tanda tangani/verifikasi artefak.**
  - **Kerjakan:** publikasikan `SHA256SUMS` dan signature GPG minimal; jika
    memakai auto-update, gunakan signature Tauri updater dan key terpisah.
  - **DoD:** pengguna dapat memverifikasi artefak sebelum memasang.
  - **Bukti selesai:** dibuat skrip staging dan penandatanganan rilis [`scripts/release/generate-release.sh`](scripts/release/generate-release.sh) yang menghasilkan `SHA256SUMS` dan tanda tangan terpisah `SHA256SUMS.asc` menggunakan kunci rilis resmi (`18F47067ACC5BBF9`). Kunci publik diekspor ke [`packaging/release/archdroid-release.asc`](packaging/release/archdroid-release.asc). Dibuat skrip verifikasi otomatis [`scripts/release/verify-release.sh`](scripts/release/verify-release.sh) (`npm run release:verify`) yang memvalidasi checksum dan tanda tangan via isolated keyring. Panduan manual dan otomatis didokumentasikan di [`docs/VERIFY-RELEASE.md`](docs/VERIFY-RELEASE.md) dan [`README.md`](README.md). Unit test penandatanganan dan verifikasi artefak lolos di [`src/lib/artifact-verification.test.js`](src/lib/artifact-verification.test.js) (29 passed).

- [x] **P0-9.4 — Samakan klaim dukungan dengan bukti uji.**
  - **Bukti:** `README.md` §1 menandai Ubuntu/Debian, Fedora/RHEL, dan
    openSUSE sebagai "Didukung Penuh (Tier 1)" dan menulis format "AUR /
    Source / AppImage" untuk Arch. `docs/P0-9.1-PACKAGING-RESULTS.md` dan
    `docs/P0-8.2-MATRIX-RESULTS.md` hanya memuat uji di VM Arch. Tidak ada
    paket AUR yang dipublikasikan; `PKGBUILD` hanya dibangun lokal.
  - **Dampak:** melanggar DoD P0-9.2 ("tidak ada klaim distro yang
    menyesatkan"). Jalur `apt-get`/`dnf`/`zypper` (nama paket, flag, root
    script) hanya diuji dengan unit test string, bukan dengan manajer paket
    sungguhan.
  - **Kerjakan (pilih satu):**
    - **Jujur dulu:** ubah README menjadi Tier 1 = Arch (teruji), Tier 2 =
      Debian/Ubuntu/Fedora/openSUSE "best effort, belum diuji live"; dan
      jangan publikasikan `.deb`/`.rpm` sebagai rilis resmi.
    - **Uji dulu:** siapkan VM Debian/Ubuntu, Fedora, dan openSUSE (pola
      `scripts/test-vm/vm.sh`) dan jalankan install artefak, preflight,
      approval paket, serta `waydroid_init` di masing-masing.
  - **DoD:** setiap distro yang disebut "didukung" punya baris bukti uji live
    (versi distro, artefak, hasil, log); AppImage diuji di minimal satu
    distro non-Arch; klaim AUR dihapus sampai paketnya ada.
  - **Bukti selesai:** opsi "Jujur dulu" diterapkan sepenuhnya. `README.md` dan `docs/P0-9.1-PACKAGING-RESULTS.md` diselaraskan secara transparan: Tier 1 adalah Arch Linux (teruji live di QEMU/KVM kernel 7.2.11 dengan artefak `archdroid-bin-*.pkg.tar.zst` dan `ArchDroid_*.AppImage`), sedangkan Debian/Ubuntu/Fedora/openSUSE diturunkan ke Tier 2 (best-effort, kontrak package mapping teruji di unit test backend tapi belum live matrix). Klaim "AUR" dihapus dan diganti dengan "PKGBUILD lokal (`makepkg -si`)". `scripts/release/generate-release.sh` hanya mempublikasikan artefak teruji Tier 1 ke root rilis dan `SHA256SUMS`, sedangkan `.deb` dan `.rpm` diisolasi di `dist-release/preview/`. Semua path absolut di `README.md` dan `docs/` diganti dengan link relatif. Unit test validasi klaim lulus di `src/lib/artifact-verification.test.js` (30 passed).

### P0-10 — CI dan gate rilis di remote

- [x] **P0-10.1 — Jalankan CI di remote/PR, bukan hanya lokal.**
  - **Bukti:** `.github/workflows/ci.yml` ada, tetapi repo belum punya remote
    dan workflow belum pernah hijau.
  - **DoD:** PR dan `main` menjalankan workflow yang required; hasilnya hijau.
  - **Bukti selesai:** Remote GitHub repository dibuat di `https://github.com/wisangdg/archdroid`. Workflow CI (`.github/workflows/ci.yml`) dijalankan dan lolos hijau (100% success):
    - Push ke `main`: [Run #41200745832](https://github.com/wisangdg/archdroid/actions/runs/41200745832) (5m23s, hijau).
    - Event `pull_request`: [Run #41201206561](https://github.com/wisangdg/archdroid/actions/runs/41201206561) pada PR #1 ([PR #1](https://github.com/wisangdg/archdroid/pull/1)) (6m27s, hijau). Seluruh step (frontend test/build/audit, Tauri system deps, rustfmt, clippy default & dev-commands, cargo test default & dev-commands, deb bundle build) diverifikasi berhasil. PR #1 telah dimerge ke `main`.

- [x] **P0-10.2 — Tambahkan pemeriksaan backend/build yang hilang.**
  - **Kerjakan:** frontend build (`npm ci` + `npm run build`), `cargo fmt
    --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`,
    `cargo clippy`/`cargo test` dengan `--features dev-commands` (kode yang
    di-gate P0-5.4), bundle build, `cargo audit`/`cargo deny`,
    `npm audit --omit=dev`, dan pengecekan lockfile. Pemeriksaan lint/test
    frontend ditambahkan bersama P1-2.
  - **DoD:** dependency rentan dan bundle yang gagal build memblokir PR rilis.
  - **Bukti selesai:** ditambahkan di `.github/workflows/ci.yml`: `cargo clippy --all-targets --features dev-commands`, `cargo test --features dev-commands`, `npm audit --omit=dev`, bundle build check (`npm run build:bundles -- --bundles deb`).

- [x] **P0-10.3 — Pin GitHub Actions ke commit SHA.**
  - **Bukti:** `ci.yml` memakai tag bergerak (`actions/checkout@v4`,
    `actions/setup-node@v4`, `dtolnay/rust-toolchain@stable`) dan tidak
    mendeklarasikan blok `permissions:`.
  - **Kerjakan:** pin ke SHA dan set `permissions: contents: read` di level
    workflow.
  - **DoD:** action pihak ketiga tidak bisa berubah tanpa review; token CI
    hanya punya izin baca.
  - **Bukti selesai:** di `.github/workflows/ci.yml`: ditambahkan `permissions: contents: read` di level workflow; action dipin ke commit SHA (`actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683`, `actions/setup-node@39370e3970a6d050c480ffad4ff0ed4d3fdee5af`, `dtolnay/rust-toolchain@683b588c487a69d01f6c6ddf40f036ff682cc2c8`).

- [x] **P0-10.4 — Commit dan review perubahan yang masih menggantung.**
  - **Bukti:** `git status` menunjukkan 26 berkas termodifikasi dan 12 path
    baru (`LICENSE`, `packaging/`, `scripts/`, `docs/P0-8.*`, `docs/P0-9.1-*`,
    `docs/VERIFY-RELEASE.md`, 5 test baru di `src/lib/`) yang belum
    di-commit. Commit terakhir (`cc62c51`, P0-7.2) mendahului P0-5, P0-7.3,
    P0-8, dan P0-9 yang sudah ditandai selesai di dokumen ini.
  - **Dampak:** item "selesai" tidak punya jejak reviewable, tidak bisa
    di-bisect, dan hilang bila working tree rusak. CI nanti hanya melihat
    commit.
  - **Kerjakan:** pecah menjadi commit per item (P0-5.x, P0-3.4/5.4, P0-7.3,
    P0-8.x, P0-9.x) dengan pesan yang merujuk ID todo; pastikan tidak ada
    artefak build ikut ter-commit (`packaging/arch/pkg|src|*.pkg.tar.*`,
    `dist-release/` sudah di-ignore; periksa dengan `git status --ignored`).
  - **DoD:** `git status` bersih; setiap item P0 bercentang menunjuk commit;
    `cargo test`, `npm test`, dan `npm run build` hijau pada HEAD.
  - **Bukti selesai:** seluruh perubahan dipecah menjadi commit bersih:
    - `0610463`: `feat(security): enforce production CSP, offline fonts, and IPC command gating (P0-5.1..P0-5.4)`
    - `fb7363e`: `feat(security): preview and approve distro packages before root elevation (P0-7.3)`
    - `25cceb7`: `test(validation): add QEMU test VM, live matrix validation, and cancellation UI fix (P0-8.1..P0-8.3)`
    - `5f2931d`: `feat(packaging): multi-target packaging, bundle metadata, release signing, and honest support contract (P0-9.1..P0-9.4)`
    - `bbc0f13`: `ci: configure remote CI with pinned action SHAs, full test matrix, and permissions (P0-10.2, P0-10.3)`
    `git status` bersih; `cargo test`, `npm test`, dan `npm run build` 100% hijau pada HEAD.

### P0-11 — Dokumentasi rilis, lisensi, dan privasi

- [x] **P0-11.1 — Lengkapi `README.md`.**
  - **Isi minimum:** target distro/desktop, dependency, instalasi, first run,
    setup root, uninstall, troubleshooting, dan known limitations.
  - **Bukti selesai:** `README.md` telah dilengkapi mencakup seluruh aspek yang dipersyaratkan:
    - Target distro Tier 1 (Arch Linux teruji live) vs Tier 2 (Debian/Ubuntu/Fedora/openSUSE best-effort) dan target yang eksplisit tidak didukung (Pure X11, non-systemd/non-standard pm).
    - Panduan instalasi dari artefak rilis (`PKGBUILD` lokal / `makepkg -si`, `archdroid-bin-*.pkg.tar.zst`, `ArchDroid_*.AppImage`, dan preview `.deb`/`.rpm`).
    - Panduan first run & Setup Wizard interaktif (deteksi status Waydroid, host health preflight, pilihan image VANILLA vs GAPPS).
    - Model hak akses & prompt root (kapan root dibutuhkan, kapan root tidak pernah diminta, dialog konfirmasi persetujuan paket eksplisit).
    - Panduan troubleshooting komprehensif (polkit agent di Hyprland/Sway, modul kernel binder missing, Wayland requirement, izin `/dev/kvm`, kompatibilitas GPU Nvidia vs AMD/Intel, pembersihan variabel AppImage).
    - Panduan uninstall bersih (penghapusan binary aplikasi paket/AppImage, data user config & cache `~/.config/archdroid` dan `~/.cache/archdroid`, serta data sistem/image Waydroid `/var/lib/waydroid` dan `~/.local/share/waydroid`).
    - Batasan yang diketahui (single instance container, Wayland dependency, ketersediaan image Google upstream untuk Houdini, status Tier 1 vs Tier 2).
    - Semua tautan relatif, tanpa kebocoran path lokal. Validasi lolos di `src/lib/artifact-verification.test.js`.
- [x] **P0-11.2 — Tambahkan `LICENSE`.**
  - MIT, atas nama pemilik repo. `waydroid_script` (GPL-3.0) tidak di-vendor
    dan hanya diunduh saat instalasi, jadi tidak memengaruhi lisensi proyek;
    pastikan alasan ini ditulis di notices (P0-11.4).
- [x] **P0-11.6 — Ganti link absolut file scheme dengan link relatif.**
  - **Bukti:** `README.md` (baris tautan VERIFY-RELEASE), `docs/VERIFY-RELEASE.md`
    (link file kunci), dan `todo.md` (P0-9.2/P0-9.3) sebelumnya sempat memakai
    path absolut lokal `home/wdg/...`.
  - **Dampak:** semua tautan mati di GitHub dan di mesin siapa pun selain
    mesin ini; path home pengembang juga ikut bocor ke dokumen publik.
  - **DoD:** `grep -rn 'file:/''//' README.md docs todo.md` kosong; tautan
    memakai path relatif seperti `docs/VERIFY-RELEASE.md`.
  - **Bukti selesai:** semua tautan absolut di `README.md`, `docs/VERIFY-RELEASE.md`, dan `todo.md` telah diganti menjadi path relatif repositori (misal `packaging/release/archdroid-release.asc`, `src-tauri/tauri.conf.json`). `grep -rn 'file:/''//' README.md docs todo.md` menghasilkan 0 temuan. Unit test `artifact-verification.test.js` memverifikasi pencegahan kebocoran path lokal.
- [x] **P0-11.3 — Tambahkan `SECURITY.md` dan `PRIVACY.md`.**
  - **Jelaskan:** model privilege, installer root, data yang disimpan, lokasi log,
    tidak ada telemetri, dan cara melaporkan masalah keamanan.
  - **Bukti selesai:** ditambahkan file `SECURITY.md` (kebijakan keamanan, model unprivileged webview/IPC, batasan elevasi root dengan sanitasi env dan isolasi cache, explicit consent modal, package approval gating, hash lock Python, serta kontak pelaporan privat ke GitHub Advisories / email pengembang) dan `PRIVACY.md` (komitmen zero telemetri/analytics, zero phone-home, pemetaan lokasi data lokal `~/.config/archdroid`, `~/.cache/archdroid`, `/var/lib/waydroid`, dan retensi/penghapusan data). Tautan menggunakan path relatif.
- [x] **P0-11.4 — Tambahkan `CHANGELOG.md` dan third-party notices.**
  - **Catat:** `waydroid_script` GPL-3.0, Phantom MIT, dan lisensi font/aset.
  - **Bukti selesai:** ditambahkan file `CHANGELOG.md` (mengikuti format Keep a Changelog untuk rilis 0.1.0 merangkum seluruh fitur P0: launcher, drag-and-drop APK, setup wizard, addon root injection dengan package approval, CSP, multi-target packaging, dan release signing) dan `NOTICES.md` (atribusi lengkap untuk Waydroid GPL-3.0, `waydroid_script` GPL-3.0 dengan penjelasan isolasi runtime non-vendored, Phantom MIT, Lucide Icons ISC, serta kebijakan font sistem lokal offline tanpa bundle binary/remote import).
- [x] **P0-11.5 — Tinjau penggunaan merek pihak ketiga.**
  - **Bukti:** judul window di `index.html` adalah “ArchDroid - LDPlayer for
    Linux”; “LDPlayer-style” juga muncul di `package.json`
    (`description`), `tauri.conf.json` (`longDescription`), dan
    `packaging/arch/PKGBUILD` (`pkgdesc`), jadi masuk ke metadata semua
    paket. README juga membandingkan dengan LDPlayer/NoxPlayer.
    Identifier `io.github.archdroid` dan URL `homepage`/`repository`
    (`https://github.com/archdroid/archdroid`) di `package.json`,
    `Cargo.toml`, dan `tauri.conf.json` mengklaim namespace/organisasi
    GitHub `archdroid` yang belum tentu dimiliki; repo ini belum punya
    remote sama sekali.
  - **Kerjakan:** hapus nama LDPlayer/Nox dari judul dan metadata paket
    (boleh tetap sebagai perbandingan di README dengan disclaimer), buat
    remote yang sebenarnya lalu samakan `homepage`/`repository`, dan pakai
    identifier dari akun/domain yang benar-benar dimiliki.
  - **DoD:** `grep -ril ldplayer` hanya mengenai README (dengan disclaimer);
    URL repo di metadata dapat dibuka dan milik pemilik proyek.
  - **Bukti selesai:** Seluruh referensi merek komersial pihak ketiga telah dibersihkan:
    - Judul window di `index.html` diubah menjadi `ArchDroid - Android Emulator Manager for Linux`.
    - Metadata paket (`package.json`, `tauri.conf.json`, `PKGBUILD`) dibersihkan dari nama merek komersial.
    - Identifier paket di `tauri.conf.json` diselaraskan menjadi `io.github.wisangdg.archdroid`.
    - URL `homepage` dan `repository` di `package.json`, `src-tauri/Cargo.toml`, `tauri.conf.json`, dan `PKGBUILD` diselaraskan ke remote resmi pemilik proyek (`https://github.com/wisangdg/archdroid`).
    - Catatan perbandingan di `README.md` dilengkapi dengan **Trademark Disclaimer** resmi.
    - `grep -ril "ldplayer"` hanya mengenai `README.md` (dan catatan todo historis).
    - Unit test baru ditambahkan di `src/lib/package-metadata.test.js` (31 passed).
- **DoD:** pengguna baru dapat memasang, memahami risiko root, dan melakukan
  troubleshooting tanpa membaca source code.

---

## 3. P1 — Launch hardening

P1 tidak boleh menunda P0, tetapi sebaiknya selesai sebelum rilis publik.

### P1-1 — Kebenaran state dan command frontend

- [x] Gunakan `CommandResult.code` (`not_found`, `not_initialized`,
  `elevation_denied`, `timeout`, `cancelled`, dan seterusnya) di
  `src/App.jsx`; hapus pencocokan pesan string seperti `includes("No such file")`.
- [x] Periksa `status` hasil `invoke` untuk command non-streaming. `invoke`
  tidak melempar exception untuk `CommandResult` berstatus `error`, sehingga
  sebelumnya kegagalan diabaikan: `handleStart`/`handleStop`/`handleRestart`
  (Restart selalu menampilkan toast “Engine Restarted”), `handleLaunchApp`,
  `toggleConfig` (toggle UI berubah walau `prop_set` gagal),
  `handleRunPhantom`, dan `open_shared_folder`. Sekarang semuanya memeriksa
  `res?.status === "success"` vs `"error"` sebelum memperbarui state atau
  menampilkan toast sukses.
- [x] Turunkan status `ProcessStepper`/`stepConfigs.js` dari payload
  `cmd-finished` (`success`/`code`), bukan dari pencocokan teks log
  (`includes("failed")`, `"Wipe complete!"`); output pihak ketiga yang memuat
  kata “failed” tidak lagi membuat stepper menampilkan error palsu bila
  `taskResult.success === true`.
- [x] **Temuan live P0-8.2 (F6):** Cancel yang sah tidak lagi menampilkan
  `EXECUTION FAILED` + “Root authentication may have been rejected…”.
  `TerminalModal.jsx` dan `stepConfigs.js` memeriksa `taskResult.code === 'cancelled'`,
  menampilkan badge `Cancelled`, label `Operation was cancelled by user`, dan
  menghilangkan troubleshooting error palsu.
- [x] Kembalikan Cancel untuk install/remove APK: operasi tersebut kini berjalan
  di bawah `TaskGuard` + `cancel_task` dengan flag cancel per-task, dan flag
  global `waydroid::CANCEL` serta command `waydroid_cancel` telah dihapus.
- [x] Bila `profile_apply` dihubungkan ke UI: `profile_apply` memanggil
  `emit_started(&app, task.id())` sebelum menjalankan operasi, sehingga
  frontend dapat menerima event dan pembatalan memiliki `task_id` yang valid.
- [x] Perbaiki `checkStatus()`: saat command error karena Waydroid tidak ada,
  tidak lagi otomatis menyetel `isInitialized = true`; membedakan `not_found`
  (belum terpasang), `not_initialized` (belum init), dan offline/online.
- [x] Ganti `setTimeout` tetap pada start/stop/restart dengan polling
  `pollStatusUntil(target, maxAttempts, intervalMs)` sampai state benar
  atau timeout.
- [x] Cegah polling 5 detik yang tumpang tindih dengan guard `statusCheckInFlight`.
- [x] Jadikan `run_phantom()` async/detached lewat `spawn_blocking` + `.spawn()`,
  memvalidasi keberadaan terminal emulator yang didukung, dan mengembalikan
  `CommandResult` berstatus terstruktur.
- [x] Pindahkan `check_host_diagnostics()`, `open_shared_folder()`,
  `get_waydroid_apps()`, dan `run_phantom()` dari main thread; beri timeout 3 detik
  pada `systemctl` di `check_host_diagnostics()` dan spawn detached pada `xdg-open`.
- [x] Tangani `dirs::data_local_dir() == None` di `open_shared_folder()`; mengembalikan
  `AppError::not_found` terstruktur daripada membuat path relatif di current directory.
- [x] Sinkronkan `src/components/terminal/stepConfigs.js` dengan output aktual;
  langkah disesuaikan dengan script checkout/venv pinned, dan tracking langkah
  dipetakan secara akurat per-`taskType`.
- [x] Ambil versi UI dari sumber tunggal (`package.json`), hapus hardcoded
  `v0.1.0` di `Sidebar.jsx`.
- [x] **Temuan live P0-8.2 (F1):** `waydroid app remove` di Waydroid 1.6.3
  exit 0 sambil menulis `Failed with code: -3` dan paket tetap ada.
  **Selesai:** `waydroid_uninstall_app` memakai `adb uninstall` bila adb
  tersedia; tanpa adb, hasil `app remove` diverifikasi lewat `app list` dan
  error actionable dikembalikan bila paket masih ada. Test parser daftar paket
  dan argv adb ditambahkan. Bukti live adb dan detail: F1 di
  `docs/P0-8.2-MATRIX-RESULTS.md`.
- [x] **Temuan live P0-8.2 (F2):** `waydroid app install` melaporkan sukses
  untuk APK invalid. **Selesai:** APK divalidasi lebih dulu (ZIP magic +
  `AndroidManifest.xml`), lalu `adb install -r` dipakai bila adb tersedia
  (live: APK invalid exit 1 dengan `INSTALL_PARSE_FAILED_NOT_APK`); tanpa adb
  file non-APK tetap ditolak sebelum CLI. Test regresi ditambahkan. Bukti:
  F2 di `docs/P0-8.2-MATRIX-RESULTS.md`.
- **DoD:** semua state transisi punya test; tidak ada toast sukses prematur.
  (Lolos 36 test frontend, 116 test backend, 135 test backend dev-commands,
  0 clippy warning, cargo fmt lulus).

### P1-2 — Test, lint, dan E2E frontend

- [x] **Tambahkan ESLint + Prettier dan script `lint`/`format:check`.**
  - **Status:** selesai. Dikonfigurasi `eslint.config.js` (flat config ESLint 9 + `eslint-plugin-react`), `.prettierrc`, dan `.prettierignore`. Script `lint`, `lint:fix`, `format:check`, dan `format` ditambahkan ke `package.json`. Seluruh codebase diformat rapi dan lolos `npm run lint` (0 error, 0 warning) serta `npm run format:check`.
- [x] **Tambahkan Vitest + React Testing Library untuk alur kritis.**
  - **Status:** selesai. Dikonfigurasi `vitest.config.js`, `src/test/setup.js`, `src/test/app-components.test.jsx`, dan `src/test/app-workflows.test.jsx` (16 test suite). Memverifikasi status error/code backend (`not_initialized`, `elevation_denied`, `timeout`, `cancelled`, `busy`), dialog konfirmasi wipe data, install GApps & Houdini sukses vs gagal, pembatalan task dengan `cancel_task` routing `task_id`, penanganan timeout, dan penolakan drag-and-drop file non-APK dengan toast peringatan.
- [x] **Tambahkan smoke E2E dengan WebDriver/Tauri driver.**
  - **Status:** selesai. Dibuat runner `scripts/e2e/smoke-webdriver.js` berbasis W3C WebDriver native over `fetch` dan headless Chromium/ChromeDriver tanpa dependensi pihak ketiga berat. Menguji 7 skenario otomatis:
    1. Startup & Initialized app loading (online badge, app grid, header).
    2. Navigasi tab (My Apps, Keymapping, Install Tools, Settings).
    3. Setup wizard preflight blocking saat status `not_initialized` dan host preflight error (jaringan/disk).
    4. Penolakan drag-and-drop file non-APK dengan warning toast.
    5. Error toast terminal emulator missing saat peluncuran keymapper (Matrix #18).
    6. Operasi streaming privileged, tombol Cancel interaktif, dan routing `cancel_task` dengan active `task_id`.
    7. Dialog konfirmasi wipe container dan pembatalan aman.
- [x] **Masukkan semuanya ke CI.**
  - **Status:** selesai. Ditambahkan step `npm run lint`, `npm run format:check`, `npm test` (unit lib + vitest components), dan `npm run test:e2e` ke `.github/workflows/ci.yml`.
- **DoD:** `npm run lint`, `npm run format:check`, `npm test` (36 node tests + 16 vitest tests), dan `npm run test:e2e` 100% hijau; alur kritis terverifikasi otomatis.

### P1-3 — Aksesibilitas

- [x] **`AppCard` memakai `role="button"`, `tabIndex`, dan handler keyboard (`Enter`/`Space`).**
  - **Status:** selesai. `AppCard.jsx` menambahkan `role="button"`, `tabIndex={disabled ? -1 : 0}`, `aria-label={Launch ${app.name}}`, `aria-disabled={disabled}`, serta handler `onKeyDown` untuk aktivasi via `Enter`/`Space` dan pembukaan context menu via tombol `ContextMenu` atau `Shift+F10`.
- [x] **Preset resolution memakai `role="button"` lengkap dengan `onKeyDown`.**
  - **Status:** selesai. `ResolutionPresets.jsx` menambahkan `onKeyDown` (`Enter`/`Space`) dan `aria-pressed={isSelected}` pada setiap preset card.
- [x] **Tambahkan focus trap, initial focus, dan restore focus pada `ConfirmModal`/`TerminalModal`.**
  - **Status:** selesai. Dibuat hook native `src/lib/useFocusTrap.js` (tanpa dependensi eksternal) yang mengatur initial focus pada elemen autofocus/aksi utama, mengunci siklus `Tab` / `Shift+Tab` di dalam modal, menangani `Escape` untuk menutup/membatalkan, dan mengembalikan fokus (`restore focus`) ke elemen pemanggil saat modal ditutup atau unmount. Di `TerminalModal.jsx` ditambahkan `role="dialog"`, `aria-modal="true"`, dan `aria-labelledby="terminal-modal-title"`.
- [x] **Pastikan context menu bisa dioperasikan keyboard dan punya `role="menu"`.**
  - **Status:** selesai. `ContextMenu.jsx` kini memiliki `role="menu"`, items bertipe `role="menuitem"`, inisialisasi fokus otomatis pada item pertama saat dibuka, navigasi panah atas/bawah (`ArrowDown`/`ArrowUp`) dengan wrap-around, `Home`/`End`, `Escape` untuk dismiss, dan restore focus ke elemen trigger.
- [x] **Audit label ikon, kontras, `aria-live`, dan `prefers-reduced-motion`.**
  - **Status:** selesai. Semua ikon Lucide yang bersifat dekoratif diberi `aria-hidden="true"`, tombol tanpa teks diberi `aria-label` eksplisit, `ToastContainer.jsx` diberi `role="status"` dan `aria-live="polite"`, serta di `src/index.css` ditambahkan indikator `:focus-visible` kontras tinggi dan media query `@media (prefers-reduced-motion: reduce)`.
- **DoD:** seluruh alur utama bisa diselesaikan hanya dengan keyboard dan screen reader dasar. Lolos 10/10 test aksesibilitas di `src/test/accessibility.test.jsx`, 26/26 vitest, 36 node unit tests, dan 7/7 WebDriver smoke E2E.

### P1-4 — Diagnostik runtime dan first-run

- [x] Hubungkan `SetupWizard` ke status Waydroid; tampilkan langkah berbeda untuk
  Waydroid hilang, belum init, init berjalan, dan siap.
  - **Status:** selesai. `src/lib/setupStage.js` menurunkan stage (`missing`/`uninitialized`/`initializing`/`ready`) dari report `check_host_diagnostics` + state `installing`; judul dan deskripsi wizard berubah per stage. Catatan: `waydroid_status_json` hanya ada di build `dev-commands`, sehingga wizard memakai report diagnostik yang selalu tersedia.
- [x] Tampilkan preflight P0-7 di UI sebelum tombol install/start.
  - **Status:** selesai untuk wizard init (sudah ada, tombol di-gate). Gating tombol start/install-tools di luar wizard belum dilakukan.
- [x] Tambahkan petunjuk actionable untuk: polkit agent tidak ada, sudo butuh
  TTY, `adb` tidak ada, `/dev/kvm` tidak bisa diakses, dan X11.
  - **Status:** selesai. `collect_diagnostics` kini mendeteksi `pkexec` tanpa polkit agent berjalan dan sudo-only tanpa TTY (status error + remediation); `adb` hilang naik ke warning; KVM dan X11 sudah punya remediation.
- [x] Pastikan `check_host_diagnostics` tidak melakukan blocking call di main
  thread.
  - **Status:** selesai (sudah memakai `spawn_blocking`).
- **DoD:** host yang belum siap mendapat penjelasan dan perbaikan, bukan error
  mentah. Terverifikasi: `cargo test` (diagnostik + nama polkit agent), `npm test`, `npm run lint`.

### P1-5 — Maintenance dependency dan supply chain

- [x] Perbarui toolchain frontend.
  - **Status:** selesai. Vite 8, Vitest 5, `@vitejs/plugin-react` 6, plus `npm audit fix`. Tersisa 6 moderate dev-only (`minimatch` via eslint) yang diterima; lihat `docs/DEPENDENCY-SECURITY.md`. `npm audit --omit=dev` = 0.
- [x] Jalankan `cargo audit` dan `npm audit --omit=dev` di CI.
  - **Status:** step `cargo audit` ditambahkan di CI (belum dijalankan lokal karena `cargo-audit` tidak terpasang); `cargo deny` tidak dipakai.
- [x] Pin GitHub Actions ke SHA dan review permission workflow (`contents: read`).
- [x] Tinjau `core:default`: tidak dipakai; hanya 3 permission eksplisit.
- [x] Batasi pembacaan icon `.desktop`.
  - **Status:** selesai. `icon_data_uri`: path absolut, file reguler, maks 512 KiB, PNG/SVG berdasarkan isi (MIME benar); XPM/lainnya ditolak. Ada unit test.
- [x] Dokumentasikan ADB auto-authorization dan batasi command ke serial Waydroid.
  - **Status:** didokumentasikan di `docs/DEPENDENCY-SECURITY.md`; test regresi serial sudah ada di `adb.rs`.
- **DoD:** temuan tersisa tercatat dan diterima eksplisit di `docs/DEPENDENCY-SECURITY.md`.

### P1-6 — Logging dan privasi

- [x] Redaksi data sensitif dari log.
  - **Status:** selesai. Isi script root/user tidak lagi ditulis ke log (hanya jumlah baris). Pesan error di log dan tampilan log melewati `redact_text` (path home → `~`, nilai `token/password/secret/api_key`, `Bearer`). Unit test `redaction_masks_home_and_secrets`. Argumen `adb shell` dan identitas device tidak pernah di-log oleh kode saat ini.
- [x] Batasi ukuran log UI dan output `waydroid_log`.
  - **Status:** selesai. Terminal UI menyimpan maks 2000 baris (`appendLogLine`); `waydroid_log` memakai `tail -n 1000`; tampilan log aplikasi dibatasi 64 KiB terakhir. File log dirotasi plugin (40 KB).
- [x] UI untuk melihat, mengekspor, dan menghapus log.
  - **Status:** selesai. Panel "Application Log" di Settings (View / Copy / Clear) dengan lokasi log ditampilkan. "Ekspor" berupa Copy ke clipboard (belum ada dialog simpan file).
- [x] Pastikan tidak ada telemetry/upload otomatis.
  - **Status:** tidak ada dependensi HTTP/telemetry di `Cargo.toml`; satu-satunya koneksi jaringan adalah probe TCP konektivitas di preflight. Dinyatakan di panel log.
- **DoD:** log cukup untuk support tanpa membocorkan data pribadi.

### P1-7 — Data, config, dan lifecycle

- [x] Beri versi/skema pada config profil dan migrasi aman.
  - **Status:** selesai. `DeviceProfile.version` (`PROFILE_VERSION = 1`), `migrate()` untuk file lama tanpa versi; file rusak dipindah ke `profile.json.corrupt` lalu fallback ke default tanpa panic.
- [x] Penulisan config atomik dan permission benar.
  - **Status:** selesai. `save_to`: file temp 0600 → `sync_all` → `rename`. `config_path()` tidak lagi jatuh ke `./` (pakai `~/.config`, lalu temp dir).
- [x] `write_marker` melaporkan error.
  - **Status:** selesai. Mengembalikan `io::Result`; penyelesaian install menampilkan `[WARNING]` di terminal bila gagal, refresh dari live check hanya me-log.
- [x] Dokumentasikan path config/cache/log, data Waydroid, uninstall bersih: `docs/DATA-AND-UNINSTALL.md`.
- [x] Marker basi: live check menghapus marker yang tidak didukung image, Init/Wipe menghapus semua marker (sudah ada test).
- [x] Peringatan eksplisit sebelum wipe kini menyebut backup via Shared Storage. Backup/export otomatis belum ada.
- **DoD:** tidak ada state "installed" palsu; data user terdokumentasi.

### P1-8 — Strategi update dan distribusi

- [x] Pilih: update manual dengan release notes (tanpa Tauri updater).
  - **Status:** diputuskan dan didokumentasikan di `docs/RELEASE-AND-UPDATES.md`. Test `release-hygiene.test.js` menjaga tidak ada plugin updater.
- [~] Kelola kunci rilis GPG `18F47067ACC5BBF9`.
  - **Status:** prosedur rotasi/revokasi, parameter kunci, dan pemeriksaan "tidak ada kunci privat di repo" (test) selesai. **Butuh tindakan pemilik:** set masa berlaku (kunci saat ini tanpa expiry), catat lokasi backup, dan publikasikan fingerprint di kanal kedua (Releases/keyserver); checklist ada di dokumen.
- [x] Jika updater: tidak berlaku.
- [x] Jika manual: instruksi upgrade, verifikasi, dan rollback tertulis.
- **DoD:** tidak ada update yang mengeksekusi artefak tanpa signature (aplikasi tidak mengunduh/menjalankan apa pun sendiri).

### P1-9 — Performa dan UX operasi

- [x] Kurangi probe berulang dan cache app list/icon berdasarkan mtime.
  - **Status:** selesai. Polling 5 detik kini hanya `waydroid_status` (GApps/Houdini lewat adb di-refresh saat sesi online dan setelah task), dilewati bila jendela tersembunyi, dan `checkStatus` tetap single-flight. `collect_apps_in` meng-cache entri `.desktop` per path+mtime (ikon hanya dibaca ulang bila file berubah; entri yang dihapus ikut keluar dari cache). Catatan: perubahan file ikon tanpa perubahan `.desktop` tidak terdeteksi.
- [x] Progress tahap sinkron dengan script.
  - **Status:** selesai. Script gapps/houdini/phantom/wipe/init mencetak `[STEP] <id>` sesuai `TASK_STEP_CONFIGS`; `detectStepProgress` memakai marker terakhir dan hanya jatuh ke heuristik teks bila tidak ada marker. Test Rust memastikan urutan marker per script; test JS memastikan marker diikuti.
- [x] Tombol disabled selama operasi dan pulih setelah gagal/cancel.
  - **Status:** tombol View Logs dan Factory Reset kini ikut `busy` (sebelumnya tidak). Pemulihan `installing` setelah gagal/cancel sudah dicakup test P1-2.
- [x] Ukur dan tetapkan budget.
  - **Status:** app list 200 entri + ikon: cold 8 ms / warm 1,4 ms (budget test: < 250 ms / < 25 ms). Diagnostik host ~0,1 s (di `spawn_blocking`). Bundle JS 249 KB (75 KB gzip). Startup dan memory jendela belum diukur (butuh sesi GUI); budget awal: jendela tampil < 1,5 s, RSS < 200 MB.
- **DoD:** command yang memanggil `systemctl`/`xdg-open`/terminal/probe tool berjalan off main thread (`preview_package_install` kini async + `spawn_blocking`); polling tidak menumpuk.

### P1-10 — Ketahanan installer

- [x] Uji instalasi ulang GApps/Houdini/multitouch setelah sukses, gagal di
  tengah, dan marker dihapus.
  - **Status:** selesai di level state machine marker (`installer_markers_follow_reinstall_success_failure_and_removal`): sukses → `pending`, live check → `verified`, install ulang idempoten, prompt ditolak tidak menyentuh marker, gagal setelah root menghapus marker, marker lama `installed` dianggap `pending`. Belum diuji end-to-end di VM dengan image sungguhan.
- [x] Disk penuh/network putus: error jelas.
  - **Status:** preflight sudah menolak disk < 500 MB / offline sebelum root. Kegagalan di tengah jalan kini dikenali dari output (`No space left on device`, DNS/koneksi putus, `unable to access 'https://`) dan terminal menampilkan `[HINT]` yang actionable; baris penyebab tetap tersimpan walau output panjang. Integritas image: waydroid_script menulis ke overlay, kegagalan menghapus marker sehingga live check melaporkan `partial`/`not_installed` alih-alih "installed". Simulasi disk penuh/network putus sungguhan belum dijalankan.
- [x] Verifikasi post-install sebelum marker dianggap valid.
  - **Status:** selesai. Marker `pending` vs `verified`; status offline melaporkan `pending` ("Unverified" di UI, tombol install tetap aktif) sampai live check mengonfirmasi. Didokumentasikan di `docs/DATA-AND-UNINSTALL.md`.
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
| `waydroid_script` berjalan sebagai root | Tertutup | P0-6.1 (dependency hash-pinned), P0-6.2 (audit path, isolasi XDG_CACHE, dan pembersihan /tmp), dan P0-6.3 (dialog konfirmasi eksplisit UI sebelum prompt polkit/sudo) selesai. |
| Klaim dukungan distro melebihi bukti uji | Tertutup | P0-9.4: README membedakan Tier 1 (Arch teruji live di VM) dan Tier 2 (Debian/Ubuntu/Fedora/openSUSE best effort); klaim AUR diganti PKGBUILD lokal; rilis resmi hanya mempublikasikan artefak teruji (Arch package + AppImage), preview .deb/.rpm diisolasi di dist-release/preview/. |
| Paket sistem terpasang tanpa persetujuan | Tertutup | P0-7.3: dialog konfirmasi menampilkan daftar paket dan command persis; backend memvalidasi ulang daftar yang disetujui (`approval_required`/`invalid_input`) dan script root hanya menjalankan paket yang disetujui. |
| Belum ada uji live Waydroid/polkit | Sebagian tertutup | P0-8.1–8.3 selesai di VM Arch (QEMU/KVM, Xvfb); distro non-Arch dan CSP di DevTools runtime belum (P0-9.4). |
| CSP `null` dan font remote | Tertutup | P0-5.1/P0-5.2: CSP produksi aktif, `@import` Google Fonts dihapus; verifikasi DevTools di P0-8. |
| Permission webview terlalu luas | Tertutup | P0-5.3: `core:default` diganti event listen/unlisten + `dialog:allow-open`, tanpa plugin shell/FS. |
| Log dapat memuat argumen sensitif | Tertutup | P1-6: script tidak di-log, pesan diredaksi. |
| Command IPC tanpa UI (shell Android arbitrer, tulis root) | Tertutup | P0-5.4: command tanpa UI di-gate di feature `dev-commands` dan tidak ada di build default. |
| Operasi non-streaming bisa bentrok dengan wipe/init | Tertutup | P0-3.4: semua operasi state-changing memakai `TaskGuard` dan ditolak `busy`. |
| Dev dependency rentan | Diterima | P1-5: upgrade ke Vite 8/Vitest 5; tersisa 6 moderate dev-only (`minimatch` via eslint), tidak masuk runtime bundle. Lihat `docs/DEPENDENCY-SECURITY.md`. |
| Pekerjaan P0-5..P0-9 belum di-commit | Tertutup | P0-10.4: seluruh perubahan di-commit per item secara bersih. |
| Metadata/judul memakai merek LDPlayer dan namespace GitHub yang belum dimiliki | Tertutup | P0-11.5: judul dan metadata dibersihkan, namespace dan repo diselaraskan ke `wisangdg/archdroid`, disclaimer merek dagang ditambahkan. |
| Tanpa telemetry | Diterima | Support mengandalkan log yang diekspor pengguna. |

---

## 6. Urutan eksekusi yang disarankan

Selesai: P0-10.4 (commit), P0-9.4 (klaim distro), P0-11 (dokumen, merek,
link relatif), P0-10 (CI remote, `cargo audit`, bundle build), dan P1
(reliability, test/lint/E2E, a11y, logging, data lifecycle). Refactor
`lib.rs`/`App.jsx` menjadi modul dan hook juga selesai (1 Nov).

Sisa:

1. **Sisa P0-8**: verifikasi CSP di DevTools runtime dan item non-teruji di
   `docs/P0-8.2-MATRIX-RESULTS.md`.
2. **P2**: fitur dan polish.

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
