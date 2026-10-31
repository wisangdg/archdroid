# ArchDroid — TODO Production Readiness

> **Status: BELUM production-ready.**
> Basis pemeriksaan: commit `7798457` (review ulang 30 Okt 2026; sebelumnya
> `639b5ec`, 24 Okt 2026).
> P0-1/P0-2 (hardening boundary privilege dan pin installer), P0-3.1
> (hasil operasi privileged ditunggu sampai proses selesai), P0-3.2
> (hasil terstruktur), P0-3.3 (single-flight dan `task_id`), P0-3.4
> (single-flight untuk semua operasi yang mengubah state), P0-4.1
> (deadline script privileged/user), P0-4.2 (cancel per task), P0-4.3
> (uji failure path), serta P0-5.1–P0-5.4 (CSP, font offline, audit webview,
> dan gate command IPC) sudah selesai. Review 30 Okt juga menambahkan
> beberapa temuan di P1.
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
- [x] `cargo test` lulus: **92 test** pada build default dan **112 test** dengan
  `--features dev-commands` (termasuk 12 test failure path P0-4.3; 20 test
  untuk modul yang di-gate hanya berjalan dengan feature itu).
- [x] `cargo clippy --all-targets -- -D warnings` lulus, baik default maupun
  `--features dev-commands`.
- [x] `npm test` lulus: **15 test** (4 isolasi event task, 4 CSP/font config,
  4 audit permukaan webview, 3 cakupan IPC command) di Node test runner.
- [x] `npm run build` lulus.
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
  bersih. Waydroid tidak tersedia/berjalan di environment pengembangan ini.
- [ ] Verifikasi CSP di DevTools runtime (tidak ada pelanggaran saat start,
  IPC, dialog, dan operasi panjang) belum dilakukan karena butuh sesi GUI;
  masuk P0-8. Pemeriksaan statis `dist/` sudah bersih.
- [ ] Uji live pkexec/sudo untuk hasil P0-3.1 (sukses, batal, exit non-zero,
  sudo gagal) tetap harus dilakukan di P0-8.
- [ ] Timeout dan Cancel per task sudah diimplementasikan, tetapi eksekusi
  live-nya (polkit prompt, installer, init, wipe) belum diuji di mesin ini;
  verifikasinya masuk P0-8.
- [ ] Dukungan distro masih ambigu: script memakai `pacman`, tetapi artefak
  yang dibangun adalah `.deb`.
- [ ] CI belum pernah berjalan di remote/PR.
- [ ] Belum ada lint frontend, test komponen, dan E2E; baru ada 15 unit test
  murni (isolasi event task, CSP/font config, audit webview, dan cakupan IPC).
- [ ] Hanya `.deb` yang dibangun; target bundle lain belum, dan instalasi
  `.deb` belum diuji di VM bersih.
- [ ] README, LICENSE, SECURITY, PRIVACY, dan CHANGELOG belum ada.
- [ ] `npm audit` penuh masih melaporkan **7 vulnerability dev-only**
  (4 high), terutama dari rantai Vite 5.4.21.
- [ ] `cargo audit`/`cargo deny` belum pernah dijalankan (tool belum terpasang
  di mesin pengembangan maupun di CI).

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
    dijalankan untuk default dan `--features dev-commands` (92 vs 112 test);
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
    `cargo clippy`/`cargo test` dengan `--features dev-commands` (kode yang
    di-gate P0-5.4), bundle build, `cargo audit`/`cargo deny`,
    `npm audit --omit=dev`, dan pengecekan lockfile. Pemeriksaan lint/test
    frontend ditambahkan bersama P1-2.
  - **DoD:** dependency rentan dan bundle yang gagal build memblokir PR rilis.

- [ ] **P0-10.3 — Pin GitHub Actions ke commit SHA.**
  - **Bukti:** `ci.yml` memakai tag bergerak (`actions/checkout@v4`,
    `actions/setup-node@v4`, `dtolnay/rust-toolchain@stable`) dan tidak
    mendeklarasikan blok `permissions:`.
  - **Kerjakan:** pin ke SHA dan set `permissions: contents: read` di level
    workflow.
  - **DoD:** action pihak ketiga tidak bisa berubah tanpa review; token CI
    hanya punya izin baca.

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
- [ ] **P0-11.5 — Tinjau penggunaan merek pihak ketiga.**
  - **Bukti:** judul window di `index.html` adalah “ArchDroid - LDPlayer for
    Linux”, dan `longDescription`/`package.json` menyebut “LDPlayer-style”.
    Identifier `io.github.archdroid` mengklaim namespace GitHub `archdroid`.
  - **Kerjakan:** hapus nama LDPlayer dari judul/metadata paket (boleh tetap
    sebagai perbandingan di README dengan disclaimer), dan pakai identifier
    dari domain/akun yang benar-benar dimiliki.
- **DoD:** pengguna baru dapat memasang, memahami risiko root, dan melakukan
  troubleshooting tanpa membaca source code.

---

## 3. P1 — Launch hardening

P1 tidak boleh menunda P0, tetapi sebaiknya selesai sebelum rilis publik.

### P1-1 — Kebenaran state dan command frontend

- [ ] Gunakan `CommandResult.code` (`not_found`, `not_initialized`,
  `elevation_denied`, `timeout`, `cancelled`, dan seterusnya) di
  `src/App.jsx`; hapus pencocokan pesan string seperti `includes("No such file")`.
- [ ] Periksa `status` hasil `invoke` untuk command non-streaming. `invoke`
  tidak melempar exception untuk `CommandResult` berstatus `error`, sehingga
  saat ini kegagalan diabaikan: `handleStart`/`handleStop`/`handleRestart`
  (Restart selalu menampilkan toast “Engine Restarted”), `handleLaunchApp`,
  `toggleConfig` (toggle UI berubah walau `prop_set` gagal),
  `handleRunPhantom`, dan `open_shared_folder`.
- [ ] Turunkan status `ProcessStepper`/`stepConfigs.js` dari payload
  `cmd-finished` (`success`/`code`), bukan dari pencocokan teks log
  (`includes("failed")`, `"Wipe complete!"`); output pihak ketiga yang memuat
  kata “failed” saat ini bisa membuat stepper menampilkan error palsu.
- [ ] Kembalikan Cancel untuk install/remove APK: masukkan operasi itu ke
  `TaskGuard` + `cancel_task`, lalu hapus flag global `waydroid::CANCEL` dan
  command `waydroid_cancel` agar hanya ada satu mekanisme cancel.
- [ ] Bila `profile_apply` dihubungkan ke UI: command ini memegang
  `TaskGuard` dan mengirim `cmd-output`/`cmd-finished`, tetapi tidak mengirim
  `cmd-started`, sehingga frontend membuang semua event-nya dan Cancel tidak
  punya `task_id`. Kirim `cmd-started` seperti `stream_privileged_script`.
- [ ] Perbaiki `checkStatus()`: saat command error karena Waydroid tidak ada,
  jangan otomatis menyetel `isInitialized = true`; bedakan “belum terpasang”,
  “belum init”, dan “offline”.
- [ ] Ganti `setTimeout` tetap pada start/stop/restart dengan polling
  `waydroid_status_json`/`booted` sampai state benar atau timeout.
- [ ] Cegah polling 5 detik yang tumpang tindih; batalkan request lama dan
  jangan biarkan respons lama menimpa state baru.
- [ ] Jadikan `run_phantom()` async/detached dan verifikasi terminal benar-benar
  ditemukan; saat ini `.output()` dapat menunggu terminal ditutup.
- [ ] Pindahkan `check_host_diagnostics()`, `open_shared_folder()`,
  `get_waydroid_apps()`, dan `run_phantom()` dari main thread (semuanya
  `fn` sinkron, jadi dijalankan Tauri di main thread); beri timeout pada
  `systemctl`/`xdg-open`.
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
  ukuran maksimum; jangan membaca file arbitrer tanpa batas. Saat ini semua
  ikon diberi MIME `image/png` secara hardcode, jadi ikon SVG/XPM rusak.
- [ ] Dokumentasikan ADB auto-authorization dan batasi command ke serial
  Waydroid (sudah ada upaya di `adb.rs`, tetapi perlu test/regresi).
- **DoD:** tidak ada temuan high/critical yang tidak tercatat dan diterima
  secara eksplisit.

### P1-6 — Logging dan privasi

- [ ] Redaksi data sensitif dari log: path home, argumen `adb shell` arbitrer,
  identitas device/profil, dan output yang mungkin berisi token.
  `run_privileged_script_for_task` saat ini menulis seluruh isi script root
  ke log file (`log::info!("root script requested:\n{script}")`), termasuk
  isi `waydroid.cfg`/`waydroid_base.prop` dari `profile_apply`.
- [ ] Batasi ukuran log UI (log terminal tidak boleh tumbuh tanpa batas) dan
  batasi output `waydroid_log`.
- [ ] Sediakan UI untuk melihat, mengekspor, dan menghapus log; dokumentasikan
  lokasinya.
- [ ] Pastikan tidak ada telemetry/upload otomatis.
- **DoD:** log cukup untuk support tanpa membocorkan data pribadi.

### P1-7 — Data, config, dan lifecycle

- [ ] Beri versi/skema pada config profil dan migrasi aman; file rusak harus
  fallback tanpa panic.
- [ ] Pastikan penulisan config atomik dan permission benar. Bukti:
  `device::save_to` memakai `fs::write` langsung (tidak tulis-temp-lalu-
  rename), dan `config_path()` jatuh ke `./` bila `dirs::config_dir()` kosong.
- [ ] `write_marker` mengabaikan error (`let _ = fs::write(...)`); kegagalan
  menulis marker harus dilaporkan agar status tidak diam-diam salah.
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
| `waydroid_script` berjalan sebagai root | Tertutup | P0-6.1 (dependency hash-pinned), P0-6.2 (audit path, isolasi XDG_CACHE, dan pembersihan /tmp), dan P0-6.3 (dialog konfirmasi eksplisit UI sebelum prompt polkit/sudo) selesai. |
| Distro target belum jelas | Tertutup | P0-7.1: multi-distro didukung eksplisit (Arch/pacman, Debian-Ubuntu/apt-get, Fedora/dnf, openSUSE/zypper); README mendokumentasikan target; installer fail-fast sebelum root pada distro yang tidak didukung. |
| Belum ada uji live Waydroid/polkit | Terbuka | P0-8: VM disposable + bukti uji. |
| CSP `null` dan font remote | Tertutup | P0-5.1/P0-5.2: CSP produksi aktif, `@import` Google Fonts dihapus; verifikasi DevTools di P0-8. |
| Permission webview terlalu luas | Tertutup | P0-5.3: `core:default` diganti event listen/unlisten + `dialog:allow-open`, tanpa plugin shell/FS. |
| Log dapat memuat argumen sensitif | Terbuka | P1-6. |
| Command IPC tanpa UI (shell Android arbitrer, tulis root) | Tertutup | P0-5.4: command tanpa UI di-gate di feature `dev-commands` dan tidak ada di build default. |
| Operasi non-streaming bisa bentrok dengan wipe/init | Tertutup | P0-3.4: semua operasi state-changing memakai `TaskGuard` dan ditolak `busy`. |
| Vite 5 dev dependency rentan | Terbuka | P1-5; tidak masuk runtime bundle, tetapi build tooling. |
| Tanpa telemetry | Diterima | Support mengandalkan log yang diekspor pengguna. |

---

## 6. Urutan eksekusi yang disarankan

1. **Tentukan scope platform** (Arch-only vs multi-distro) dan siapkan VM uji.
2. **P0-6**: supply chain installer root (seluruh P0-5 sudah selesai).
3. **P0-7 + P0-8**: preflight dependency dan validasi live end-to-end.
4. **P0-9 + P0-10 + P0-11**: artefak, CI remote, dokumentasi/legal.
5. **P1**: frontend reliability, test/lint/E2E, a11y, logging, data lifecycle.
6. **P2**: fitur dan polish.

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
