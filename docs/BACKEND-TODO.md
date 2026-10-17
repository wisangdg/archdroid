# DroidDeck — Backend TODO

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
| Infra | `CommandResult{status,data,message}`, event `cmd-output`/`cmd-finished`, `elevate_env`, `sh_quote`, `resolve_program`, `run_and_stream`, 7 unit test |

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

- [ ] **P0 S — `src/waydroid.rs`: satu modul pembungkus CLI Waydroid**
  Semua panggilan (`status`, `session start|stop`, `container restart`, `app install|uninstall|launch|list`, `prop get|set`, `shell`, `init`, `show-full-ui`) lewat sini. Satu tempat untuk bangun argv + timeout.
  *DoD:* `grep -c 'Command::new("waydroid")' src/lib.rs` == 0.

- [ ] **P0 S — Model error bertipe**
  Ganti `CommandResult{status:String}` dengan `type Result<T> = std::result::Result<T, AppError>`; `AppError` = `NotFound | NotInitialized | ElevationDenied | Timeout | Exit{code,String} | Io`.
  *DoD:* serialisasi tetap `{status,data,message}` agar UI tidak pecah; unit test untuk tiap varian → pesan.

- [ ] **P0 S — Preflight `waydroid`**
  `fn waydroid_version() -> Result<String>` + deteksi "belum `waydroid init`".
  *DoD:* test: PATH dikosongkan → `Err(NotFound)`; pesan error menyebut langkah perbaikan.

- [ ] **P0 M — Operasi panjang jadi non-blocking + bisa dibatalkan**
  Bungkus dengan `tauri::async_runtime::spawn_blocking` + simpan `Child` di `State` supaya bisa `kill`.
  *DoD:* dua operasi panjang bersamaan tidak membekukan UI; tombol Cancel benar-benar menghentikan proses.

- [ ] **P0 S — Audit injeksi shell**
  Telusuri semua script: pakai `sh_quote` untuk nilai dinamis.
  *DoD:* test dengan payload `x'; rm -rf /tmp/x; echo '` → tidak tereksekusi.

- [ ] **P1 M — Aktifkan logging terstruktur**
  `tauri-plugin-log` sudah jadi dependency tapi belum di-`init()`. Catat setiap command + durasi.
  *DoD:* ada file log berisi jejak command.

- [ ] **P1 S — Tambah unit test argv builder**
  Setiap helper murni (bangun argumen `waydroid app install`, parser `waydroid status`, dst) punya test.
  *DoD:* `cargo test` naik dari 7 → ≥15 test, tanpa butuh root.

- [ ] **P2 S — Package manager multi-distro**
  Tiga script installer masih hardcode `pacman` (`waydroid_init`, `install_gapps`, `install_houdini`). Tambah pemilih `pacman|apt-get|dnf`.
  *DoD:* fungsi `install_pkg()` di script; test bahwa pemilihan benar berdasar `command -v`.

---

## 2. Paritas CLI inti (P0–P1, murah, langsung terasa)

| LDPlayer | Fungsi DroidDeck | Padanan | Prio |
|---|---|---|---|
| `list3` | `list_instances()` → `Vec<InstanceInfo>` | agregasi status+profil | **P0 S** |
| `isrunning` | `is_running() -> bool` | parse `waydroid status` | **P0 S** |
| `reboot` | `reboot()` | `waydroid container restart` | **P0 S** |
| `quitall` | *(pakai `waydroid_stop`)* | `waydroid session stop` | **P0 S** |
| `killapp` | `kill_app(pkg)` | `am force-stop` | **P0 S** |
| `adb --command` | `adb_cmd(cmd)` | `waydroid shell` | **P0 S** |
| `getprop` (tanpa key) | `getprop_all()` | dump seluruh prop | **P0 S** |
| `launch --wait` | `wait_for_boot(timeout_ms)` | poll `sys.boot_completed=1` | **P1 S** |
| `setuplanguage` | `set_locale(tag)` | `prop set persist.sys.locale` | **P1 S** |
| — | `status_json()` | 1 objek: available/initialized/running/booted/gapps/houdini/phantom | **P1 S** |
| `launch --hide/--mini` | `show_full_ui()` / headless | mode sesi (bukan window player) | **P1 S** |
| `--directlink` | `open_uri(uri)` | `am start -a VIEW -d <uri>` | **P2 S** |

- [ ] **P0 S — `InstanceInfo` (mirror skema `list3` LDPlayer)**
  ```rust
  struct InstanceInfo { playerid: u32, name: String, running: bool,
      android_started: bool, cpu: u32, memory: u32, root: bool,
      width: u32, height: u32, dpi: u32 }
  ```
  *DoD:* `list_instances()` mengembalikan JSON siap dipakai UI/scripting; test parser dengan output `waydroid status` yang di-stub.

---

## 3. Device profile & config (P0–P1, inti "rasa LDPlayer")

Kelompok key LDPlayer sudah terekstrak (lihat blueprint §3). Ini yang dipetakan ke Waydroid.

- [ ] **P0 M — `DeviceProfile` + persistensi**
  Struct + simpan/muat JSON di config dir app. Field: `width,height,dpi,cpu,memory,root,mac,imei,imsi,androidid,manufacturer,model,phone,locale`.
  *DoD:* round-trip serialize/deserialize bertest; file rusak → fallback default, tidak panic.

- [ ] **P0 M — `profile_apply(profile)`**
  Tulis prop + `waydroid.cfg` / `waydroid_base.prop` (di bawah elevasi), idempoten.
  *DoD:* apply dua kali → hasil sama; test bahwa argv builder benar (tanpa menjalankan).

- [ ] **P1 M — `profile_get()`**
  Baca nilai efektif (prop + cfg), kembalikan struct yang sama.
  *DoD:* `profile_get(profile_apply(x)) == x` untuk field yang didukung.

- [ ] **P1 S — Resolusi/DPI/CPU/memori**
  `--resolution w,h,dpi`, `cpuCount`, `memorySize`. Mapping eksplisit mana yang Waydroid dukung.
  *DoD:* tabel mapping di kode (bukan komentar) + test.

- [ ] **P1 M — Props identitas**
  `manufacturer`, `model`, `androidid`, `imei/imsi`, `mac`.
  *DoD:* field yang **tidak** bisa di-set ditandai `Unsupported` dan UI disuruh menyembunyikannya — jangan diam-diam gagal.

- [ ] **P2 S — `--root 1|0`** (khusus 9.x) → prop/cfg terkait.
- [ ] **P2 S — `globalsetting`** (`--fps/--audio/--cleanmode`) → knob cfg; tandai yang tak ada padanan.

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
- [ ] **P1 S — CI**: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `npm run build`.
- [ ] **P1 S — Least privilege** di `capabilities/default.json` (tinjau scope `dialog`).
- [ ] **P2 S — Packaging** deb/AppImage: identifier `io.github.droiddeck`, desktop entry, ikon.
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
