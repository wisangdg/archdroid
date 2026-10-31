# ArchDroid — Backend TODO

**Berkas ini untuk kerja di `src-tauri/src/`.** Prioritas: `P0` (fondasi/blocker) → `P1` (nilai tinggi) → `P2` (pelengkap). Effort: `S` ≤ sehari · `M` beberapa hari · `L` ~1 minggu · `XL` butuh komponen Android baru.

Sumber fitur: hasil RE LDPlayer 9 (`~/Downloads/ldplayer-re/ANALYSIS-LDPlayer-CLI-config-keymapping.md`) dan `~Downloads/ldplayer-re/BLUEPRINT-DroidDeck.md`. Nama fungsi sengaja dibuat mirip `ldconsole` agar familiar.

---

## 0. Status sekarang (jangan dikerjakan lagi)

| Sudah ada | Catatan |
|---|---|
| `waydroid_status/start/stop/init/wipe` | shell langsung ke `waydroid` |
| `get_waydroid_apps` | baca `waydroid.*.desktop` + ekstrak ikon base64 |
| `waydroid_install_apk/uninstall_app/launch_app` | |
| `waydroid_prop_set/prop_get` | |
| `waydroid_log`, `open_shared_folder` | |
| `check_/install_gapps`, `check_/install_houdini`, `install_/run_phantom` | lewat `stream_privileged_script` |
| Infra | `CommandResult{status,data,message,code}`, event `cmd-started`/`cmd-output`/`cmd-finished` dengan `task_id`, `cancel_task(task_id)`, `elevate_env`, `sh_quote`, `resolve_program`, `run_and_stream` |

**Batasan bawaan yang harus diingat:** Waydroid = **1 container**, jadi tidak ada multi-instance nyata. Semua item di sini untuk instance tunggal.

---

## Aturan kerja backend (konvensi)

1. **Satu modul per area** — jangan tambah `Command::new("waydroid")` baru di luar modul `waydroid`.
2. **Semua yang butuh root** lewat `stream_privileged_script` (jangan `sudo` di dalam script).
3. **Tidak ada string yang di-`format!` ke shell tanpa `sh_quote`** — input pengguna (nama paket, path, prop) wajib di-quote.
4. Setiap fungsi murni (argv builder, parser, konversi koordinat) **wajib punya unit test** — tidak butuh root.
5. Jangan tiru telemetri LDPlayer (`UA-156094621-*`, `middledata.ldmnq.com`).

---

## 1. Fondasi (P0 — kerjakan duluan, membuka semua tahap berikut)

- [x] **P0 S — `src/waydroid.rs`: satu modul pembungkus CLI Waydroid**
  Semua panggilan (`status`, `session start|stop`, `container restart`, `app install|uninstall|launch|list`, `prop get|set`, `shell`, `init`, `show-full-ui`) lewat sini. Satu tempat untuk bangun argv + timeout.
  *DoD:* `grep -c 'Command::new("waydroid")' src/lib.rs` == 0. → **selesai**: 0 kemunculan. Yang sudah dibungkus: `status`, `session start|stop`, `app install|remove|launch|list`, `prop get|set`, `shell`, `--version`. Aksi lain (`container`, `show-full-ui`, `intent`, `logcat`) menyusul di §2.

- [x] **P0 S — Model error bertipe**
  Ganti `CommandResult{status:String}` dengan `type Result<T> = std::result::Result<T, AppError>`; `AppError` = `NotFound | NotInitialized | ElevationDenied | Timeout | Exit{code,String} | Io`.
  *DoD:* serialisasi tetap `{status,data,message}` agar UI tidak pecah; unit test untuk tiap varian → pesan. → **selesai**: `src/error.rs`; `AppError::code()` stabil; `From<AppError> for CommandResult`; `CommandResult` dapat field `code` opsional. 9 test di `error.rs`.

- [x] **P0 S — Preflight `waydroid`**
  `fn waydroid_version() -> Result<String>` + deteksi "belum `waydroid init`".
  *DoD:* test: PATH dikosongkan → `Err(NotFound)`; pesan error menyebut langkah perbaikan. → **selesai**: `Waydroid::detect()` (PATH → `NotFound`) dan `Waydroid::run` memetakan pesan "not initialized" ke `NotInitialized` **meski exit code 0**; command `waydroid_version` didaftarkan.

- [x] **P0 M — Operasi panjang jadi non-blocking + bisa dibatalkan**
  Bungkus dengan `tauri::async_runtime::spawn_blocking` + simpan `Child` di `State` supaya bisa `kill`.
  *DoD:* dua operasi panjang bersamaan tidak membekukan UI; tombol Cancel benar-benar menghentikan proses.
  *(Timeout per kelas command sudah ada: 30s cepat / 120s normal / 1800s sama seperti default waydroid.)*
  → **selesai**: command yang bisa lama (`status`, `version`, `start`, `stop`, `install`, `uninstall`, `launch`, `prop get|set`, `check_gapps/houdini`) jadi `async` + `off_main_thread` → `spawn_blocking` (Tauri menjalankan command non-`async` di **main thread**, jadi command sync lama membekukan UI). Cancel untuk `app install|remove` tetap memakai `waydroid_cancel` + flag global `CANCEL` di `waydroid.rs` yang dicek di loop poll (`run_cancellable`), child dibunuh dalam satu tick (~20ms); error `AppError::CancelRequested` (code `cancelled`) berbeda dari timeout. **Temuan**: `waydroid session start` adalah daemon foreground (`session_manager.py` diakhiri `looper.run()`) — tidak lagi pakai timeout (dulu programnya hang); sekarang grace-wait 5s: exit cepat → error, tetap hidup → sukses + reap di thread belakang. Sejak P0-3.3 backend memaksa satu task streaming aktif lewat `TaskGuard`, dan event membawa `task_id`. Sejak P0-4.1 semua script privileged/user punya deadline dan child diberi process group sendiri; timeout mengirim SIGTERM ke grup, memberi 5 detik untuk trap `EXIT` membersihkan `$ARCHDROID_WORK`, lalu SIGKILL. Sejak P0-4.2 tombol Cancel di terminal memakai `cancel_task(task_id)`: `TaskGuard` menyimpan flag cancel per task, `cmd-started` memberi tahu UI ID task aktif, dan `wait_with_deadline` mengecek flag itu. ID basi/asing adalah no-op, cancel bersifat terminal (tidak memicu fallback sudo), dan hasilnya `AppError::CancelRequested` (code `cancelled`).

- [x] **P0 S — Audit injeksi shell**
  Telusuri semua script: pakai `sh_quote` untuk nilai dinamis.
  *DoD:* test dengan payload `x'; rm -rf /tmp/x; echo '` → tidak tereksekusi. → **selesai untuk jalur waydroid**: argv dibangun sebagai elemen terpisah dan tidak pernah lewat shell (test `a_package_name_never_becomes_a_second_command`). Script root tetap memakai `sh_quote` seperti sebelumnya.

- [x] **P0 M — Batas privilege dan supply-chain installer**
  Home user untuk jalur root dibaca dari `/proc/self/status` + `/etc/passwd`/`getent`, bukan `HOME`/`PATH`/XDG/cache dari proses GUI. `pkexec`, `sudo`, `bash`, dan `getent` di-resolve dari direktori sistem absolut; root memakai `PATH` tetap, private `HOME`/`TMPDIR` di `$ARCHDROID_WORK`, serta konfigurasi Git/Python/pip user dinull-kan. Installer pihak ketiga di-pin ke commit immutable, di-fetch ke direktori `mktemp -d`, dan diverifikasi dengan `git rev-parse HEAD`; tidak ada lagi `/tmp/waydroid_script`, `/tmp/phantom`, atau `git pull` reuse. Phantom berjalan sebagai user dengan `INSTALL_SYSTEM_WRAPPER=0`, dan marker install ditulis/dihapus oleh proses app setelah root sukses — bukan oleh root ke cache user.
  *DoD:* test environment tersanitasi, resolver helper absolut, repo ter-pin, workdir privat, marker app-thread, dan semua script installer valid bash. → **selesai**; lihat `docs/THIRD-PARTY.md`. Sisa risiko: `waydroid_script` tetap berjalan sebagai root; dependency PyPI-nya sudah hash-pinned (P0-6.1), audit path/izinnya sudah selesai (P0-6.2), dan konfirmasi eksplisit UI sebelum prompt root sudah aktif (P0-6.3).

- [x] **P0 S — Hash-pin dependency Python installer**
  `gapps_install_script()` dan `houdini_install_script()` tidak lagi membaca `requirements.txt` dari checkout upstream. Lock hasil `uv pip compile --universal --python-version 3.10 --generate-hashes --only-binary :all:` disimpan di `src-tauri/resources/waydroid_script-requirements.lock` dan di-embed ke binary dengan `include_str!`, sehingga checkout tidak bisa menggantinya. Installer menulis lock itu ke workdir privat lalu menjalankan `pip install --require-hashes --only-binary=:all: -r <lock>`.
  *DoD:* setiap paket direct/transitif punya versi + hash SHA-256; tidak ada index/URL alternatif; test menjalankan setup dengan pip stub dan memeriksa argumen; lock terpasang pada Python 3.10 dan 3.15. → **selesai**; lihat `docs/THIRD-PARTY.md`.

- [x] **P1 M — Aktifkan logging terstruktur**
  `tauri-plugin-log` sudah jadi dependency tapi belum di-`init()`. Catat setiap command + durasi.
  *DoD:* ada file log berisi jejak command. → **selesai**: plugin di-`init()` pada level Info; target bawaan = stdout + `<app_log_dir>/ArchDroid.log` (rotasi 40 KB, keep-one; Linux: `~/.local/share/io.github.archdroid/logs/`). Setiap proses anak dicatat di `run_program`: `program argv ok in Nms`, `failed in Nms: <error>`, `timed out after Ns`, `cancelled after Ns`; spawn gagal di-`error!`. Script root dicatat saat diminta, dan hasil proses elevasi (pkexec/sudo) dicatat di `run_and_stream`. Command murni in-process (mis. `waydroid_cancel`) tidak punya durasi untuk dicatat. Verifikasi runtime (buka app, cek file log) belum bisa dilakukan di mesin ini karena `waydroid` tidak terpasang.

- [x] **P1 S — Tambah unit test argv builder**
  Setiap helper murni (bangun argumen `waydroid app install`, parser `waydroid status`, dst) punya test.
  *DoD:* `cargo test` naik dari 7 → ≥15 test, tanpa butuh root. → **selesai**: **27** test, semuanya tanpa root (`/bin/echo`, `/bin/sh`, `/bin/sleep` dipakai sebagai stub).

- [x] **P2 S — Package manager multi-distro**
  Tiga script installer sebelumnya hardcode `pacman` (`waydroid_init`, `install_gapps`, `install_houdini`). Modul `platform::PackageManager` kini mendeteksi `pacman|apt-get|dnf|zypper`, memetakan paket per distro (mis. `squashfs-tools` vs `squashfs`), menggunakan flag non-interaktif minimalis (`--needed`, `--no-install-recommends`), dan melakukan fast-fail sebelum prompt root jika dependensi kurang pada distro yang tidak didukung. Sejak P0-7.3 paket tidak pernah dipasang diam-diam: `preview_package_install` menampilkan daftar paket dan command persis di dialog konfirmasi, UI mengirim daftar yang disetujui, dan backend menolaknya bila tidak lengkap atau berisi paket di luar rencana.
  *DoD:* modul `platform.rs`; test pemilihan dan pemetaan paket per distro; README mendokumentasikan target yang didukung dan tidak didukung. → **selesai** (P0-7.1 / P2).

### Temuan baru saat mengerjakan §1 (diverifikasi dari sumber waydroid 1.6.3)

- **`waydroid shell` butuh root** (`actionNeedRoot` di `tools/__main__.py`), begitu juga `container …`. Artinya probe hidup untuk GApps/houdini (`shell pm list packages`, `shell getprop …`) **selalu gagal saat app jalan tanpa root** dan jawabannya sekarang bergantung pada marker. `probe()` sengaja menelan kegagalan itu. Perlu keputusan: jalankan probe lewat jalur privileged, atau pakai `adb` (`waydroid adb connect` + `adb shell`) yang tidak butuh root.
- **`container start|stop|restart` butuh root** → `reboot()` (§2) tidak bisa lewat jalur biasa, harus lewat `stream_privileged_script`/`pkexec`.
- **Aksi yang belum dibungkus**: `show-full-ui` (bukan `show full ui`), `app intent <ACTION> <URI>` (= padanan `--directlink`), `logcat`, `adb connect|disconnect`, `container freeze|unfreeze`.
- **`/var/lib/waydroid/waydroid.cfg`** menyimpan bagian `[properties]` (override prop) plus `lcd_density`, `suspend_action`, `auto_adb`, `mount_overlays`, `vendor_type`, `arch`, `images_path` → bahan §3 (`profile_apply`, `globalsetting`).
- **`CommandResult` sekarang punya field `code`** (`not_found`, `not_initialized`, `elevation_denied`, `timeout`, `exit`, `io`) sehingga frontend tidak perlu lagi mencocokkan teks pesan. Field lama tidak berubah.
- Perubahan perilaku yang disengaja: `waydroid_stop` kini melaporkan **error** kalau `waydroid session stop` keluar non-nol (dulu selalu "success" asal proses berhasil di-spawn).

---

## 2. Paritas CLI inti (P0–P1, murah, langsung terasa)

| LDPlayer | Fungsi ArchDroid | Padanan | Prio |
|---|---|---|---|
| `list3` | `list_instances()` → `Vec<InstanceInfo>` | agregasi status+profil | **P0 S** |
| `isrunning` | `is_running() -> bool` | parse `waydroid status` | **P0 S** |
| `reboot` | `reboot()` | `waydroid container restart` (**butuh root**) | **P0 S** |
| `quitall` | *(pakai `waydroid_stop`)* | `waydroid session stop` | **P0 S** |
| `killapp` | `kill_app(pkg)` | `am force-stop` | **P0 S** |
| `adb --command` | `adb_cmd(cmd)` | `waydroid shell` (**butuh root**) atau `adb shell` | **P0 S** |
| `getprop` (tanpa key) | `getprop_all()` | dump seluruh prop | **P0 S** |
| `launch --wait` | `wait_for_boot(timeout_ms)` | poll `sys.boot_completed=1` | **P1 S** |
| `setuplanguage` | `set_locale(tag)` | `prop set persist.sys.locale` | **P1 S** |
| — | `status_json()` | 1 objek: available/initialized/running/booted/gapps/houdini/phantom | **P1 S** |
| `launch --hide/--mini` | `show_full_ui()` / headless | mode sesi (bukan window player) | **P1 S** |
| `--directlink` | `open_uri(uri)` | `am start -a VIEW -d <uri>` | **P2 S** |

- [x] **P0 S — `InstanceInfo` (mirror skema `list3` LDPlayer)**
  ```rust
  struct InstanceInfo { playerid: u32, name: String, running: bool,
      android_started: bool, cpu: u32, memory: u32, root: bool,
      width: u32, height: u32, dpi: u32 }
  ```
  *DoD:* `list_instances()` mengembalikan JSON siap dipakai UI/scripting; test parser dengan output `waydroid status` yang di-stub.

**Status Phase A (20 Okt 2026): selesai.**

| Command | Hasil |
|---|---|
| `waydroid_is_running` | parse `Session:\tRUNNING`; `not_initialized` → `false` |
| `waydroid_status_json` | `available/initialized/running/booted/gapps/houdini/phantom`, degradasi (bukan error) saat container mati |
| `waydroid_list_instances` | satu entri: playerid 0, name `waydroid` |
| `waydroid_reboot` | `waydroid container restart` via helper elevasi (butuh root, streaming event) |
| `waydroid_kill_app` | `adb shell am force-stop <pkg>`; nama paket divalidasi (`invalid_input`) |
| `waydroid_getprop_all` | `adb shell getprop` |
| `waydroid_adb_cmd` | `adb shell <cmd>` bebas (fitur, bukan bug) |

Catatan:

- **A4 lewat ADB, bukan `waydroid shell`.** `waydroid shell` butuh root
  (`actionNeedRoot`), sedangkan `waydroid adb connect` tidak. Konsekuensi:
  host wajib punya `adb` (paket `android-tools`); tanpa itu command
  mengembalikan `not_found` + hint. Koneksi di-refresh tiap panggilan
  (`ponytail:` di `src-tauri/src/adb.rs`).
- **`InstanceInfo`**: `width/height/dpi` diisi dari profil perangkat (§3);
  `cpu`/`memory` memakai resource host (container inherit), `root` selalu
  `true` (shell container memang root).
- **`reboot`** memakai jalur elevasi yang sama dengan installer: hasilnya
  lewat event `cmd-output`/`cmd-finished`, bukan `CommandResult.data`.
- Nama command memakai prefix `waydroid_*` agar konsisten dengan command lama;
  `status_json`/`list_instances` mengembalikan JSON string di `data`.

---

## 3. Device profile & config (P0–P1, inti "rasa LDPlayer")

Kelompok key LDPlayer sudah terekstrak (lihat blueprint §3). Ini yang dipetakan ke Waydroid.

- [x] **P0 M — `DeviceProfile` + persistensi**
  Struct + simpan/muat JSON di config dir app. Field: `width,height,dpi,cpu,memory,root,mac,imei,imsi,androidid,manufacturer,model,phone,locale`.
  *DoD:* round-trip serialize/deserialize bertest; file rusak → fallback default, tidak panic.

- [x] **P0 M — `profile_apply(profile)`**
  Tulis prop + `waydroid.cfg` / `waydroid_base.prop` (di bawah elevasi), idempoten.
  *DoD:* apply dua kali → hasil sama; test bahwa argv builder benar (tanpa menjalankan).

- [x] **P1 M — `profile_get()`**
  Baca nilai efektif (prop + cfg), kembalikan struct yang sama.
  *DoD:* `profile_get(profile_apply(x)) == x` untuk field yang didukung.

- [x] **P1 S — Resolusi/DPI/CPU/memori**
  `--resolution w,h,dpi`, `cpuCount`, `memorySize`. Mapping eksplisit mana yang Waydroid dukung.
  *DoD:* tabel mapping di kode (bukan komentar) + test.

- [x] **P1 M — Props identitas**
  `manufacturer`, `model`, `androidid`, `imei/imsi`, `mac`.
  *DoD:* field yang **tidak** bisa di-set ditandai `Unsupported` dan UI disuruh menyembunyikannya — jangan diam-diam gagal.

- [ ] **P2 S — `--root 1|0`** (khusus 9.x) → prop/cfg terkait.
- [ ] **P2 S — `globalsetting`** (`--fps/--audio/--cleanmode`) → knob cfg; tandai yang tak ada padanan.

**Status Phase C (20 Okt 2026): selesai (P0–P1).**

| Command | Hasil |
|---|---|
| `profile_get` | JSON `DeviceProfile`; overlay `[properties]` dari `waydroid.cfg`, lalu `wm size`/`wm density` bila sesi + adb hidup |
| `profile_apply` | simpan JSON; resolusi/DPI via adb (langsung); identitas ke `[properties]` lewat helper elevasi (streaming `cmd-output`/`cmd-finished`) |
| `profile_capabilities` | tabel `field/supported/detail` untuk UI menyembunyikan field tak didukung |

Mapping (tabel `FIELDS` di `src-tauri/src/device.rs`, ada test-nya):

| Field profil | Mekanisme Waydroid 1.6.3 | Kapan berlaku |
|---|---|---|
| `width`/`height` | `adb shell wm size WxH` | langsung; override Android bertahan setelah reboot |
| `dpi` | `adb shell wm density N` | langsung |
| `manufacturer` | `ro.product.manufacturer` di `[properties]` | setelah container restart |
| `model` | `ro.product.model` di `[properties]` | setelah container restart |
| `locale` | `persist.sys.locale` di `[properties]` | setelah container restart |
| `cpu`, `memory`, `root`, `mac`, `imei`, `imsi`, `androidid`, `phone` | tidak ada knob di Waydroid | dilaporkan `unsupported`, tidak pernah gagal diam-diam |

Catatan:

- `[properties]` di `waydroid.cfg` adalah sumber persisten: `tools/helpers/lxc.py`
  menulisnya ke `waydroid_base.prop` setiap container start. Karena itu
  identitas butuh restart, sedangkan `waydroid prop set` hanya runtime dan
  tidak dipakai untuk apply.
- **Tidak ada `persist.waydroid.width/height` di 1.6.3** (dicek dari sumber);
  karena itu resolusi/DPI lewat `wm`, bukan prop. Efeknya skala UI Android,
  bukan ukuran window Wayland.
- Tulis cfg hanya jika nilai `[properties]` benar-benar berubah, jadi apply
  kedua tanpa perubahan tidak memunculkan prompt root. Payload ditulis
  sebagai base64 ke `waydroid.cfg.tmp` lalu `mv` (atomik, aman dari quoting).
- Cfg harus terbaca (mode 644). Kalau tidak terbaca, profil tetap disimpan
  tapi identitas **tidak** diterapkan dan alasannya dikembalikan di `notes`.
- `profile_get` jatuh ke nilai JSON tersimpan untuk width/height/dpi saat sesi
  mati/adb tidak ada; field identitas dibaca dari cfg dan key yang tidak ada
  berarti default sistem (bukan nilai JSON), jadi write root yang dibatalkan
  tidak tampak seolah-olah berhasil. Dengan begitu
  `profile_get(profile_apply(x)) == x` untuk field yang didukung.
- Belum ada tombol "reset ke default" (`wm size reset`/`wm density reset`);
  nilai `0` berarti "jangan sentuh", bukan reset.
- Verifikasi runtime belum bisa dilakukan: `waydroid`/`adb` tidak terpasang
  di mesin ini. Parser `wm` dan round-trip JSON/properti dites dengan output
  stub.

---

## 4. File & data (P1, dibutuhkan backup/transfer)

- [ ] **P1 M — `push_file(local)` / `pull_file(pkg, path, local)`**
  Bridge shared folder + `waydroid shell cp`; sanitasi path.
  *DoD:* test sanitasi path (`../`, null byte, spasi).

- [ ] **P1 M — `backup_app(pkg, dest)` / `restore_app(pkg, file)`**
  tar `/data/data/<pkg>` + apk-nya (mirror `backupapp/restoreapp`).
  *DoD:* arsip bisa dibuka ulang; nama file diberi timestamp.

- [ ] **P1 L — `backup_all(dest)` / `restore_all(file)`**
  Arsipkan data dir Waydroid (mirror `backup/restore`, `.ldbk` ≈ tar) + progres streaming.
  *DoD:* progress tampil di UI; restore memverifikasi checksum dulu.

- [ ] **P2 S — `list_shared_folder()`**
- [ ] **P2 S — Metadata APK sebelum instal** (`aapt`/`aapt2`): nama paket, label, ikon → dialog konfirmasi.

---

## 5. Keymapping & input (P1–P2, fitur ikonik)

Catatan penting: CachyDroid tidak memiliki surface render Waydroid, jadi injeksi input harus eksplisit. **Phantom** sudah dipakai lewat `install_phantom`/`run_phantom` — backend tinggal menyediakan model mapping + injeksi dasar.

- [ ] **P1 M — Primitive input**
  `input_tap(x,y)`, `input_swipe(x1,y1,x2,y2,ms)`, `input_text(s)`, `input_key(keycode)` → `waydroid shell input`.
  *DoD:* test bahwa argv tidak bisa di-inject lewat `s`/angka.

- [ ] **P1 M — Model mapping + simpan/muat**
  `MappingLayout { keys: Vec<MappingKey> }`, `MappingKey { id, kind: Point|Disc|Drag|Macros, x, y, radius, key, raw }` — meniru 7 kelas `ClassJoystick*` dan 17 template `xml_mapping_*` LDPlayer. Simpan JSON, dukung import/export.
  *DoD:* skema bertest; file contoh valid ditulis ke `docs/`.

- [ ] **P1 L — Penangkapan input host + konversi koordinat**
  Overlay/hotkey global → hitung koordinat device dari posisi kursor, lalu injeksi (`input` atau `ydotool`/uinput).
  *DoD:* fungsi konversi koordinat punya test tabel (termasuk letterbox/aspect mismatch).

- [ ] **P2 S — 25 hotkey default** (daftar `hotkeySettings.*` LDPlayer): screenshot, rotate, volume +/−, back/home/menu, fullscreen, dll → bind ke aksi.
- [ ] **P2 XL — Helper APK guest untuk multitouch**
  `waydroid shell input` **tidak mendukung multitouch**; untuk paritas LDPlayer penuh butuh APK helper (AccessibilityService/`sendevent`) — persis pola `ldmnq.apk` LDPlayer. Spike + dokumen desain dulu.

---

## 6. Otomasi / makro (P2)

- [ ] **P2 M — Registry quick action** (`action --key/--value`): screenshot, rotate, volume, dst.
- [ ] **P2 L — Rekam & putar ulang** (`operatelist/operateinfo/operaterecord`): skema JSON meniru `{recordInfo, file, createTime, info, code, error}`, ekstensi `.record`.

---

## 7. Hardening & rilis (P1–P2)

- [ ] **P1 S — Semua command tahan saat `waydroid` tak ada / belum di-init** (tidak panic, pesan jelas).
- [x] **P1 S — CI**: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `npm run build`.
  → **selesai**: `.github/workflows/ci.yml` (ubuntu-24.04: `npm ci` + `npm run build`, lalu `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`). Workflow belum pernah jalan (repo belum punya remote); CI baru hijau setelah file frontend yang belum ter-commit ikut masuk.
- [x] **P1 S — Least privilege** di `capabilities/default.json` (tinjau scope `dialog`).
  → **selesai**: `dialog:default` → `dialog:allow-open`, karena frontend hanya memakai `open()` (`src/App.jsx`). `core:default` dipertahankan sebagai baseline Tauri; memangkasnya per-API hanya menambah noise tanpa model ancaman yang jelas.
- [ ] **P2 S — Packaging** deb/AppImage: identifier `io.github.archdroid`, desktop entry, ikon.
- [ ] **P2 S — First-run wizard** disambungkan ke `status_json()`.

---

## 8. Non-goals (jangan dikerjakan)

- **Multi-instance VM** (`add/copy/remove/rename/list2`) — Waydroid satu container. Dokumentasikan sebagai tidak didukung.
- **IPC window-message Windows**, registry Windows — spesifik platform.
- **Telemetri/store LDPlayer** — tidak ditiru.

---

## 9. Urutan yang disarankan

```
P0 fondasi (§1: waydroid.rs, error, preflight, audit injeksi)
  └─→ P0 CLI inti (§2: list_instances, is_running, reboot, kill_app, adb_cmd)
        └─→ P0/P1 device profile (§3)
              └─→ P1 files & backup (§4)
                    └─→ P1 keymapping v1 (§5, tanpa helper APK)
                          └─→ P2 makro (§6) / helper APK (§5 XL)
Hardening (§7) berjalan paralel di setiap tahap.
```

**Titik mulai yang paling menguntungkan:** §1 `waydroid.rs` + model error. Sesudah itu semua command baru hanya menambah fungsi kecil, dan test jadi mudah karena argv builder terpisah dari eksekusi.
