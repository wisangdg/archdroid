# ArchDroid — Rencana Improvement

> **Dibuat:** 9 Nov 2026. **Review terakhir:** 25 Nov 2026 (§15).
> **Arsip:** review 9, 10, dan 15 Nov (§1–§14, semua item A–H, R1–R22, G0–G15)
> ada di [`docs/archive/IMPROVEMENT-PLAN-2026-11.md`](docs/archive/IMPROVEMENT-PLAN-2026-11.md)
> dengan nomor bagian dan ID yang sama. Backlog backend awal ada di
> [`docs/archive/BACKEND-TODO.md`](docs/archive/BACKEND-TODO.md), dan gate rilis
> 0.1.x di [`docs/archive/RELEASE-GATE-0.1.md`](docs/archive/RELEASE-GATE-0.1.md).
> Berkas ini satu-satunya daftar pekerjaan yang hidup.
> **Effort:** `S` ≤ sehari · `M` beberapa hari · `L` ~1 minggu.
> **Prioritas:** `P1` sebelum/bersama rilis berikutnya · `P2` beberapa minggu ke
> depan · `P3` saat menyentuh area itu atau bila ada permintaan.


## Status pengerjaan

> **Per 25 Nov 2026:** review §15 dikerjakan pada hari yang sama. Semua PR yang
> terbuka (#8, #23–#31) dan PR baru (#32–#34) sudah di-merge ke `main`
> setelah CI hijau. CI tidak lagi merah: advisory `url` dan `cookie` sudah
> ditutup (R23), dan `main` kini diaudit setiap minggu (C5).
>
> Tiga perbaikan keamanan dan data masuk sebelum merge:
> - #27: root hanya menjalankan salinan Phantom yang hash-nya disetujui (R24),
>   dan daemon yang tertinggal dihentikan (R25).
> - #28: assistant tidak hilang kalau penulisan gagal di tengah (R26).
> - #23: paket inti Android tidak bisa dihapus (R27).
>
> Block Ads mendapat switch AdGuard DNS (E6, diuji di VM). Untuk keluhan kabur
> dan lag di game (E7) sudah ada alat ukur, tetapi pengukurannya butuh game
> yang dimainkan di host.
>
> **Rilis 0.1.11 terbit 25 Nov** (B5, [rilis](https://github.com/wisangdg/archdroid/releases/tag/v0.1.11)),
> setelah smoke di VM; pengguna tidak lagi tertahan di 0.1.6. Branch yang sudah
> di-merge dihapus dan auto-delete dinyalakan (C8).
>
> **Menunggu:** AUR (B4: PKGBUILD 0.1.11 siap dan lolos `makepkg -f`, tetapi
> belum ada akun AUR), dan pengukuran E7 di host.

**Hasil uji 25 Nov (`main` di `f20f1cf`):** `cargo test` 234 lulus (253 dengan `dev-commands`; 9 `#[ignore]`), `node --test` 76 (+1 skip: `dist-release` belum di-stage), Vitest 149, E2E 7/7; clippy (kedua set fitur), fmt, ESLint, dan Prettier bersih; `npm audit --omit=dev` 0, `cargo audit` tanpa kerentanan (4 peringatan yang diizinkan).

### To-do

Urut prioritas. ⬜ belum · ⏳ menunggu tindakan atau keputusan maintainer ·
❌ diputuskan tidak dikerjakan. Item yang selesai pindah ke bagiannya dengan ✅
dan tanggal.

| # | Pekerjaan | Prio | Status |
|---|---|---|---|
| E7 | Game kabur saat bergerak dan terasa lag | P2 | ⏳ alat ukur siap (`scripts/perf/game-frames.sh`); jalankan di host saat bermain (§15) |
| B4 | Publikasi `archdroid-bin` ke AUR | P2 | ⏳ `packaging/aur/PKGBUILD` sudah 0.1.11 dan lolos `makepkg -f` terhadap rilis (biner identik dengan `.deb` bertanda tangan). Tersisa: akun AUR dengan SSH key (pendaftaran ditutup sementara sejak 9 Nov), lalu `scripts/release/publish-aur.sh` |
| E1 | Patch `fake_touch` upstream ([PR #60](https://github.com/waydroid/android_vendor_waydroid/pull/60)) | P2 | ⏳ menunggu review upstream |
| F5 | Uji di GNOME, KDE, dan GPU AMD | P2 | ⏳ butuh mesin lain |
| §13 | Ukur di hardware ber-GPU dan cek `document.hidden` di Hyprland | P3 | ⏳ butuh hardware |
| E3 | Touch mode multi-monitor | P3 | ⏳ uji bersama pengguna |
| C7 | ESLint 10 | P3 | ⏳ menunggu `eslint-plugin-react` (25 Nov: 7.37.5 masih `eslint ^9.7`) |
| E8 | Host Diagnostics: GPU yang benar-benar dipakai Waydroid | P3 | ⬜ (§15) |
| R25 | Sisa: handler SIGTERM untuk daemon Phantom | P3 | ⬜ opsional; penanda di R25 sudah menangani crash dan kill pada start berikutnya |
| G13–G15 | `exec-out` untuk screenshot, blur overlay keymap, siklus adb saat boot | P3 | ⬜ saat menyentuh area itu (arsip §14) |

**Backlog fitur** (dari `BACKEND-TODO`, belum dijadwalkan; kerjakan bila ada
permintaan):
- **P2 S:** daftar shared folder.
- **P2 S:** metadata APK (nama, ikon) sebelum instal.
- **P2 S:** knob `--root` dan `globalsetting` (FPS, audio).
- **P2 S:** 25 hotkey bawaan (screenshot, rotate, volume, back/home).
- **P1 M:** `push_file`/`pull_file`.
- **P2 M:** quick action.
- **P2 L:** rekam dan putar ulang.
- **P2 XL:** helper APK untuk multitouch.
- **P1 S, verifikasi:** semua command tahan saat `waydroid` tidak ada atau
  belum di-init.
- ❌ (E5, 9 Nov): backup data app (`backup_app`/`backup_all`).
- ❌ (§9): primitive input dan model mapping sendiri.

---

## 9. Sengaja tidak direkomendasikan sekarang

- **Auto-updater:** keputusan di `docs/RELEASE-AND-UPDATES.md` tetap benar (app
  meminta root; CSP memblokir jaringan). Jalur update Tier 1 adalah B4.
- **Flathub:** sandbox bertentangan dengan pkexec, Waydroid di host, dan
  `/dev/uinput`; hampir semua perintah harus lewat `flatpak-spawn --host`.
- **Migrasi TypeScript penuh dan i18n:** belum ada kebutuhan yang terlihat,
  biayanya tinggi.
- **Multi-instance:** Waydroid hanya punya satu container (arsip `BACKEND-TODO` §8).
- **Keymapper buatan sendiri** (model mapping, arsip `BACKEND-TODO` §5): Phantom dan touch
  mode sudah menutup kebutuhan saat ini; tunggu permintaan.
- **Mengganti `CommandResult` dengan enum di wire:** kontraknya stabil dan
  teruji; manfaatnya kecil dibanding perubahan di semua komponen.
- **Virtualisasi daftar app:** diukur 10 Nov dengan 300 app (payload 2 MB):
  39 ms script per 30 detik saat idle, 78 ms untuk 10 ketikan di search.
- **Title bar buatan sendiri** (menyatukan title bar jendela dengan top bar,
  yang sama-sama menulis "ArchDroid"): hanya menghemat satu baris, sedangkan
  tombol jendela dan drag harus dibuat ulang dan diuji di setiap desktop.
- **Tema terang:** belum ada permintaan, dan token warna harus rapi dulu (H3).
- **Memecah `index.css`:** pemindaian 9 Nov tidak menemukan kelas yang tidak
  dipakai (kelas dinamis seperti `toast-${type}` ikut dihitung); belum ada masalah
  nyata yang terselesaikan dengan memecahnya.
---

## 11. Aturan untuk setiap item

- Perbaikan bug dimulai dengan test yang gagal, seperti pola P0-4.3 di `docs/archive/RELEASE-GATE-0.1.md`.
- Satu item, satu commit atau lebih dengan Conventional Commits, seperti riwayat
  repo ini.
- Perubahan yang terlihat pengguna masuk `CHANGELOG.md` di `[Unreleased]`; klaim
  README hanya diubah bersama bukti uji.
- Command IPC baru masuk build produksi hanya bersama UI dan test-nya; selain itu
  tetap di balik `dev-commands`.
- Setelah item selesai, centang di sini dengan tanggal dan bukti singkat, seperti
  di `docs/archive/RELEASE-GATE-0.1.md`.
---

## 15. Review 25 Nov 2026: rilis, CI, dan antrean PR

Diperiksa:
- `fix/keymap-apply-message` di `8f54221` (PR #27), ditambah perubahan yang
  belum di-commit di `src/lib/keymapProfile.js`.
- `main` di `6199895`.
- PR, run CI, dan rilis di GitHub.

- **Cara:**
  - Test lokal.
  - `gh` untuk PR, log CI, dan rilis.
  - Membaca diff empat PR fitur. Setiap temuan PR dicek ulang ke kodenya
    sebelum ditulis di sini.
  - Tidak ada jendela yang dibuka di desktop, dan VM tidak dijalankan.
- **Hasil uji lokal (branch ini):**
  - `cargo test` 214 lulus (233 dengan `dev-commands`; 7 `#[ignore]`).
  - `node --test` 77, Vitest 136.
  - clippy, fmt, ESLint, dan Prettier bersih; `npm audit --omit=dev` 0.
  - E2E tidak dijalankan. `cargo audit` tidak terpasang di mesin ini, jadi
    buktinya dari CI.

### Gambaran

Kondisi sebelum dikerjakan; hasil per item ada di bagiannya (**Hasil**).

Kodenya sehat. Semua item §3–§14 sudah selesai, kecuali yang menunggu hardware
atau akun. Yang macet sekarang adalah jalur dari kode ke pengguna:

1. **Pengguna masih di 0.1.6.**
   - Rilis GitHub terakhir 0.1.6 (9 Nov).
   - Hanya `v0.1.7` yang punya tag, dan tag itu tidak punya rilis.
     `v0.1.8`–`v0.1.10` belum di-tag.
   - Artefak 0.1.7 yang sudah ditandatangani masih tertinggal di `dist-release/`.
   - README sudah menawarkan Block Ads dan editor keymap (0.1.9), padahal
     keduanya belum ada di rilis mana pun.
   - Perbaikan besar 0.1.7 juga belum sampai ke pengguna: Launch saat engine mati
     merusak sesi, dan CPU 92–99%.
2. **CI merah untuk setiap PR baru.**
   - Run 23 Nov untuk #29–#31 lulus semua test (Rust 207/226), lalu gagal hanya
     di `cargo audit`: `url` 2.5.8, RUSTSEC-2026-0592, diperbaiki di ≥ 2.5.9.
   - `main` akan ikut merah pada push berikutnya.
   - #27 dan #28 masih terlihat hijau hanya karena run terakhirnya 18 Nov.
3. **10 PR terbuka.**
   - Empat PR fitur sudah menunggu 7–10 hari: #23 dan #24 sejak 15 Nov, #27 dan
     #28 sejak 18 Nov.
   - Enam lainnya dari Dependabot; yang tertua, #8, sejak 10 Nov.
   - `git merge-tree` tidak menemukan konflik di antara keempat PR fitur.
4. **Dokumen mulai basi.**
   - `docs/BACKEND-TODO.md` terakhir diubah 9 Nov. Packaging dan First-run wizard
     masih tercatat belum, dan nama lama "CachyDroid" masih dipakai.
   - Rencana ini sendiri sudah 1255 baris, dan sebagian besar isinya selesai.
5. **Laporan pemakaian 23 Nov** (main game di host ini). Belum ada item yang
   menangani ketiganya:
   - Iklan masih sempat muncul walau Block Ads aktif. Setelah Private DNS
     AdGuard dipasang, tes berikutnya bersih.
   - Clash Royale kabur saat layar digeser, padahal screenshot di host tajam.
   - Game terasa lag.

### Ringkasan

| ID | Item | Prio | Effort | Status |
|---|---|---|---|---|
| R23 | CI: `url` 2.5.8 kena RUSTSEC-2026-0592 | P1 | S | ✅ 25 Nov (#32, juga `cookie`) |
| R24 | #27: pkexec menjalankan `~/.local/bin/phantom`, yang bisa ditulis user, sebagai root | P1 | S–M | ✅ 25 Nov (#27, diuji di VM) |
| R25 | #27: keyboard bisa tetap di-grab; Stop yang gagal tidak terlihat | P2 | S | ✅ 25 Nov (#27); SIGTERM ⬜ opsional |
| R26 | #28: assistant terhapus sebelum nilai lamanya disimpan | P2 | S | ✅ 25 Nov (#28) |
| R27 | #23: app sistem penting bisa dihapus tanpa peringatan | P2 | S | ✅ 25 Nov (#23) |
| B5 | Rilis 0.1.11: semua perubahan sejak 0.1.6 sekaligus | P1 | M | ✅ 25 Nov ([rilis](https://github.com/wisangdg/archdroid/releases/tag/v0.1.11), smoke di VM) |
| C5 | Audit dependensi terjadwal di `main` | P2 | S | ✅ 25 Nov (#33) |
| C6 | Action GitHub naik versi major (#31) | P2 | S | ✅ 25 Nov (#31) |
| C7 | ESLint 9 sudah tidak didukung | P3 | S–M | ⏳ menunggu `eslint-plugin-react` |
| C8 | Kebersihan repo: branch yang sudah di-merge, peringatan Cargo | P3 | S | ✅ 25 Nov (Cargo #33; branch dihapus) |
| D6 | Satu backlog yang hidup: BACKEND-TODO dan arsip rencana ini | P2 | S | ✅ 25 Nov |
| D4 | `lib.rs` tumbuh lagi: 1875 → 2294 baris | P3 | M | ✅ 25 Nov (#27) |
| E6 | Block Ads lapis kedua: Private DNS opsional | P2 | M | ✅ 25 Nov (#34, diuji di VM) |
| E7 | Game kabur saat bergerak dan terasa lag | P2 | S (ukur) | ⏳ alat ukur siap; pengukuran di host oleh pengguna |
| E8 | Host Diagnostics: GPU yang benar-benar dipakai Waydroid | P3 | S | ⬜ (temuan E7) |

**Urutan** (langkah 1–3, 5, dan 6 (D4) ✅ 25 Nov; langkah 4 ⏳):
1. Kerjakan R23 di branch sendiri dari `main`, lalu jalankan ulang CI #29–#31.
2. Merge PR Dependabot yang hijau:
   - #25 dan #26.
   - #29 dan #30 bersama-sama.
   - #8, setelah ikon dicek di Chromium headless.
3. Merge PR fitur:
   - #24 apa adanya.
   - #23 setelah R27.
   - #28 setelah R26.
   - #27 setelah R24, R25, dan WIP `keymapProfile.js` di-commit.
4. B5 (0.1.11), lalu B4 dengan PKGBUILD 0.1.11.
5. C5, C6, dan D6; pengukuran E6 dan E7.
6. P3, saat menyentuh area itu: C7, C8, D4, G13 `exec-out`, G14, dan G15.

### R23 — CI merah karena advisory `url` (P1, S)
- **Bukti:** log `cargo audit` pada run 23 Nov (#29, #30, #31):
  - `error: 1 vulnerability found!`
  - `Crate: url`, `ID: RUSTSEC-2026-0592`
  - `Solution: Upgrade to >=2.5.9`

  `src-tauri/Cargo.lock` masih memakai 2.5.8.
- **Kerjakan:** `cargo update -p url`, lalu satu commit `build(deps)` dari
  `main`. Dry run 25 Nov: hanya `url` yang naik, 2.5.8 → 2.5.9.
- **DoD:**
  - `cargo audit` hijau di CI.
  - Dependabot me-rebase #29–#31, dan kalau masih gagal, penyebabnya ada di isi
    PR itu sendiri.
- **Hasil (25 Nov) ✅, #32:**
  - `url` 2.5.9. Run pertama PR itu lalu memunculkan advisory berikutnya:
    `cookie` 0.18.1 dari `tauri` (RUSTSEC-2026-0594), jadi ikut naik ke 0.18.3.
  - `cargo audit` 0.22.2 lokal bersih, hanya 4 peringatan yang diizinkan.
  - Setelah merge, CI #29–#31 hijau tanpa perubahan pada isinya.

### R24 — #27: root menjalankan biner yang bisa ditulis user (P1, S–M)
- **Bukti:**
  - Install Phantom memasang `phantom` ke `$HOME/.local/bin`. Filenya milik
    user, mode 755 (`scripts.rs:309-315`).
  - `find_phantom` (`lib.rs:1629`) mencari di `PATH`, lalu di `TRUSTED_DIRS`,
    lalu di `~/.local/bin`.
  - Hasilnya dipakai `phantom_daemon::start` (`phantom_daemon.rs:166`), yang
    menjalankan biner itu lewat pkexec sebagai root.
  - `TRUSTED_DIRS` (`elevate.rs:17`) ada justru supaya program root tidak
    diambil dari direktori yang bisa ditulis user. PR ini melanggar aturan itu.
  - Penanganan environment-nya sudah benar: `env -i`, nama variabel divalidasi,
    `sh_quote`.
- **Skenario:**
  1. Proses apa pun yang berjalan sebagai user mengganti
     `~/.local/bin/phantom`.
  2. Pada Save & Apply berikutnya, pengguna melihat prompt polkit yang memang
     ia tunggu.
  3. Begitu prompt disetujui, biner pengganti itu berjalan sebagai root, tanpa
     tanda apa pun di prompt.
- **Kerjakan:** biner yang dijalankan sebagai root jangan diambil dari file
  yang bisa ditulis user.
  - Skrip root sendiri yang menyalin biner ke direktori milik root, misalnya
    `/usr/local/lib/archdroid/phantom`.
  - Skrip itu memeriksa SHA-256 salinan tadi terhadap hash v1.1.0 yang sudah
    di-pin di installer, lalu menjalankan salinan itu.
  - Pemeriksaan dari sisi user sebelum pkexec tidak cukup, karena file masih
    bisa ditukar setelah dicek (TOCTOU).
  - Kalau hash-nya lain (misalnya Phantom dipasang sendiri), tolak dan arahkan
    pengguna ke `sudo -E phantom --daemon` di terminal.
- **DoD:**
  - Test: skrip root tidak pernah menjalankan biner dengan hash lain. Argv
    builder dan skripnya diuji dengan cara yang sama seperti `elevate`.
  - Live di VM: Save & Apply tetap jalan dengan Phantom yang dipasang
    ArchDroid.
- **Hasil (25 Nov) ✅, #27:**
  - `keymap_apply` menjawab `APPROVAL_REQUIRED` dengan program, SHA-256-nya,
    dan apakah itu rilis 1.1.0 yang di-pin (`PHANTOM_BIN_SHA256`).
  - Editor menampilkan panel persetujuan, lalu memanggil lagi dengan hash yang
    disetujui. Panelnya ada di dalam editor, bukan `ConfirmModal`, karena dua
    focus trap di `window` saling berebut Escape.
  - Skrip root menyalin program ke `/usr/local/lib/archdroid/`, memeriksa hash
    salinan, lalu menjalankan salinan itu.
  - Test `only_a_copy_with_the_approved_hash_is_started` menjalankan skrip itu
    terhadap program tiruan.
  - **VM (Phantom 1.1.0, Waydroid jalan):**
    - Hash salah: exit 65, dan tidak ada file yang tertinggal.
    - Hash benar: salinan `root:root 0755`, daemon berjalan sebagai root dengan
      `exe=/usr/local/lib/archdroid/phantom`, dan IPC di socket user.
    - `phantom status` dan `phantom shutdown` sebagai user tetap jalan.
  - **Catatan host ini:** `~/.local/bin/phantom` sudah bukan rilis 1.1.0 (diganti
    23 Nov 16:03; rilis asli di `~/.local/share/phantom/backup/`). Save & Apply
    akan menampilkan hash-nya dengan tombol "Run This Phantom as Root".
  - **Sisa risiko (desain Phantom, sama dengan `sudo -E phantom --daemon`):**
    daemon root membaca `~/.config/phantom/config.toml` milik user.

### R25 — #27: keyboard tetap di-grab; Stop gagal tanpa pesan (P2, S)
- **Bukti:**
  - Daemon dihentikan saat app keluar lewat `RunEvent::Exit` (`lib.rs:2037`).
    Event ini tidak terjadi kalau ArchDroid crash atau dibunuh dengan SIGTERM.
  - Daemon berjalan sebagai root di process group sendiri. Jadi ia tetap hidup
    dan tetap meng-grab semua keyboard.
  - `stopDaemon` (`KeymapView.jsx:86-95`) tidak membaca `CommandResult` dari
    `phantom_daemon_stop`. Kalau Stop gagal, banner "Phantom is running" tetap
    tampil tanpa penjelasan.
- **Kerjakan:**
  1. Tampilkan pesan gagal dari `phantom_daemon_stop` sebagai toast.
  2. Saat ArchDroid dibuka dan engine mati, hentikan daemon yang tercatat
     dimulai oleh ArchDroid. Pencatatannya pakai penanda di `markers`, bukan
     hanya `STARTED_HERE` di memori.
  3. Opsional: handler SIGTERM yang memanggil `stop_if_started_here`.
- **DoD:**
  - Test komponen untuk toast saat Stop gagal.
  - Test unit untuk penanda start/stop.
  - Live: `kill -TERM` ArchDroid saat daemon jalan, buka lagi, dan keyboard
    sudah bebas.
- **Catatan:** engine stop memang menghentikan daemon apa pun, termasuk yang
  dimulai dari terminal. Itu disengaja dan sudah tertulis di CHANGELOG, jadi
  bukan temuan.
- **Hasil (25 Nov) ✅, #27:**
  - Stop yang gagal kini menampilkan toast.
  - "Dimulai oleh ArchDroid" dicatat di
    `~/.cache/archdroid/phantom-daemon.started`, bukan lagi flag di memori.
    Saat ArchDroid dibuka dengan engine mati, daemon peninggalan dihentikan.
  - Handler SIGTERM tidak dibuat (⬜ opsional), karena penanda sudah menangani
    crash dan kill pada start berikutnya.
  - `kill -TERM` live belum diuji.

### R26 — #28: assistant hilang permanen kalau langkah kedua gagal (P2, S)
- **Bukti:**
  - `apply` (`assistant.rs:84-91` di `feat/super-key-assistant`) mengosongkan
    `ASSISTANT`, lalu `VOICE_INTERACTION`.
  - Nilai lama baru dikembalikan ke pemanggil, lalu disimpan, kalau kedua
    langkah itu berhasil.
  - Kalau `put` kedua gagal, `?` keluar lebih dulu. Assistant Android sudah
    kosong, tetapi `previous_assistant` tidak pernah disimpan, jadi mematikan
    opsi ini tidak bisa memulihkannya.
- **Kerjakan:**
  - Simpan preferensi beserta nilai lamanya sebelum `put` pertama.
  - `super_key_sync` sebaiknya juga lewat `run_exclusive`, seperti
    `super_key_set`, karena keduanya membaca dan menulis `super-key.json`.
- **DoD:** test dengan `Secure` tiruan yang gagal di `put` kedua: nilai lama tetap
  tersimpan dan bisa dipulihkan.
- **Hasil (25 Nov) ✅, #28:**
  - Preferensi beserta nilai lamanya disimpan sebelum `put` pertama. Test-nya
    gagal di kode lama.
  - `super_key_sync` tidak dimasukkan ke `run_exclusive`: sync saat boot tidak
    dicoba ulang, jadi jawaban `busy` akan membuatnya hilang sepanjang sesi.
    Sebagai gantinya, sync dan switch memakai mutex untuk `super-key.json`.
  - Sisa: kalau `put` pertama berhasil dan yang kedua gagal, boot berikutnya
    tidak mengosongkan `VOICE_INTERACTION` lagi.

### R27 — #23: app sistem penting bisa dihapus tanpa peringatan (P2, S)
- **Bukti:**
  - Kalau `adb uninstall` menjawab `DELETE_FAILED_INTERNAL_ERROR`,
    `uninstall_app` (`adb.rs` di `fix/uninstall-system-apps`) mencoba lagi dengan
    `pm uninstall --user 0`.
  - Dialognya (`App.jsx:413`) tetap berbunyi "permanently removed", padahal yang
    terjadi pada app sistem adalah disembunyikan dari user 0.
  - My Apps punya filter System, jadi launcher, Settings, SystemUI, atau
    WebView bisa dihapus untuk user 0.
  - Cara memulihkannya (`cmd package install-existing`) hanya tertulis di
    CHANGELOG, tidak ada di UI.
- **Catatan:** `DELETE_FAILED_INTERNAL_ERROR` juga muncul untuk paket yang sudah
  tidak ada. Dalam kasus itu `pm uninstall --user 0` ikut gagal, jadi hanya
  pesannya yang berubah. Ini belum diverifikasi live.
- **Kerjakan:**
  - Daftar tolak kecil untuk paket inti: SystemUI, launcher, Settings, WebView,
    dan package installer.
  - Kalimat dialog khusus untuk app sistem: app disembunyikan untuk user
    Android, dan Factory Reset mengembalikannya.
- **DoD:**
  - Test: paket di daftar tolak tidak pernah sampai ke `pm uninstall`.
  - Test komponen untuk kalimat dialog app sistem.
- **Hasil (25 Nov) ✅, #23:**
  - `CORE_PACKAGES`: `android`, SystemUI, `launcher3`, Settings, dua WebView,
    dan dua package installer. Hanya `com.android.settings` yang terkonfirmasi
    ada di image Waydroid; nama lain adalah nama AOSP/LineageOS standar.
  - Dialog app sistem menjelaskan cara mengembalikannya.
  - Jalur tanpa adb (`waydroid app remove`) tidak memakai daftar ini.

### B5 — Rilis 0.1.11 (P1, M) ⏳
- **Kenapa sekarang:** lihat butir 1 di Gambaran. Setiap hari tanpa rilis,
  pengguna baru yang mengikuti Quick Start mendapat 0.1.6 dan tidak menemukan
  fitur yang dijanjikan README.
- **Usul:** 0.1.7–0.1.10 tidak perlu diterbitkan satu per satu.
  - Terbitkan 0.1.11 dari `main` setelah langkah 1–3 di Urutan.
  - Catatan rilisnya menggabungkan CHANGELOG 0.1.7–0.1.11. Perubahan yang
    menyangkut root dan privasi ditaruh paling atas: Block Ads menulis
    `/system/etc/hosts` di overlay, dan daemon Phantom berjalan lewat pkexec.
  - Opsional: tag bertanda tangan `v0.1.8`–`v0.1.10` pada commit
    `chore(release)` masing-masing, tanpa artefak, supaya riwayat cocok dengan
    CHANGELOG.
- **Langkah:**
  - Ikuti `docs/RELEASE-AND-UPDATES.md` § Building a release.
  - Pastikan `dist-release/` tidak lagi berisi artefak 0.1.7 sebelum
    `generate-release.sh` dijalankan.
- **DoD:**
  - Rilis "Latest" 0.1.11 punya `SHA256SUMS.asc`, dan `verify-release.sh`
    lulus.
  - Smoke di VM Arch dari 0.1.6 ke 0.1.11, seperti B1, termasuk Save & Apply
    keymap.
  - PKGBUILD AUR dinaikkan ke 0.1.11 (B4).
- **Hasil (25 Nov) ✅:** [ArchDroid 0.1.11](https://github.com/wisangdg/archdroid/releases/tag/v0.1.11)
  sudah terbit sebagai Latest.
  - **Tag dan artefak:** #36 di-merge, lalu tag bertanda tangan `v0.1.11` di
    `010a067` (kunci rilis, lewat `git tag -u 18F47067ACC5BBF9`, karena
    `user.signingkey` tidak di-set). Artefak dari `appimage.yml` run
    45579085213: checksum CI cocok, dan `BUILD-INFO.txt` menyebut commit tag.
    `generate-release.sh` dan `verify-release.sh` lolos.
  - **Smoke sebelum publikasi** (rilis dibuat sebagai draft dulu), di VM dari
    image smoke 0.1.6:
    - `pacman -U` 0.1.6 → 0.1.11 → 0.1.6 → 0.1.11: engine start, 11 app, dan
      stop di setiap versi.
    - Kartu AdGuard DNS tampil.
    - Save & Apply menjawab `approval_required` dengan `verified: true`; tanpa
      agen polkit, pkexec menolak dengan pesan yang benar.
    - Detail di `docs/P0-8.2-MATRIX-RESULTS.md`.
  - **Release notes:** perubahan root dan privasi sejak 0.1.6 ditulis paling atas.
  - **Tidak dikerjakan:** tag `v0.1.8`–`v0.1.10` (opsional; setiap tag `v*`
    menjalankan build paket penuh), uji di Debian dan Fedora, dan AppImage
    rilis di hardware asli.

### C5 — Audit dependensi terjadwal (P2, S)
- **Bukti:** R23 ketahuan dari PR Dependabot yang tidak ada hubungannya, bukan
  dari `main`. CI hanya jalan saat push ke `main` dan saat ada PR.
- **Kerjakan:** job kecil dengan `schedule:` mingguan dan `workflow_dispatch`, yang
  hanya menjalankan `cargo audit` dan `npm audit --omit=dev` di `main`.
- **DoD:** run terjadwal pertama muncul di Actions. Advisory baru membuat run
  `main` merah, bukan PR orang lain.
- **Hasil (25 Nov) ✅, #33:**
  - `.github/workflows/audit.yml` jalan setiap Senin 04:17 UTC dan bisa
    dipicu manual.
  - `npm audit` memakai `--package-lock-only`, jadi tidak perlu install.
  - Run manual pertama di `main`: sukses ([run 45543503692](https://github.com/wisangdg/archdroid/actions/runs/45543503692): npm dan cargo bersih).

### C6 — Action GitHub naik versi major (#31) (P2, S)
- **Isi #31:** `actions/checkout` 4 → 7, `setup-node` 4 → 7,
  `upload-artifact` 4 → 7, dan `download-artifact` 4 → 8. Di CI, PR ini hanya
  gagal karena R23.
- **Risiko:** ada di `appimage.yml`, yang membangun semua paket. Workflow itu
  jalan saat push ke `main`, saat tag, dan saat dipicu manual, tidak saat PR.
  Yang bisa rusak: nama artefak, dan `gh run download` di langkah rilis.
- **DoD:**
  - Setelah R23, CI #31 hijau.
  - `appimage.yml` dipicu manual di branch itu, dan `gh run download` dengan
    nama artefak dari `RELEASE-AND-UPDATES.md` berhasil.
- **Hasil (25 Nov) ✅, #31:**
  - `appimage.yml` dipicu manual di branch #31: job AppImage dan Arch hijau.
  - Nama artefak tidak berubah (`appimage-`/`packages-`/`arch-<sha>`).
  - `gh run download -n arch-<sha>` berhasil, dan `SHA256SUMS.arch` cocok.

### C7 — ESLint 9 sudah tidak didukung (P3, S–M)
- **Bukti:**
  - CI mencetak `npm warn deprecated eslint@9.39.5: This version is no longer
    supported`. ESLint terbaru 10.15.0.
  - Penghalangnya `eslint-plugin-react` 7.37.5, versi terbaru saat dicek 25 Nov,
    yang peer-nya `eslint ^9.7`. `eslint-plugin-react-hooks` 7.1.1 sudah
    menerima ^10.
- **Dampak:** ESLint hanya alat dev dan tidak masuk biner.
- **Kerjakan:** tunggu rilis plugin itu. Kalau terlalu lama, nilai
  penggantinya. Naik major ESLint tetap dikerjakan manual (aturan R11).
- **Hasil (25 Nov) ⏳:** dicek ulang; belum ada rilis `eslint-plugin-react`
  yang menerima ESLint 10.

### C8 — Kebersihan repo (P3, S)
- **Branch:** 10 branch di `origin` sudah di-merge ke `main`, misalnya
  `release/0.1.8`–`release/0.1.10`, `feat/keep-awake`, dan `feat/keymap-editor`.
  Opsi "Automatically delete head branches" mati. Maintainer menghapus branch
  itu dan menyalakan opsinya.
- **Cargo:** `cargo check` memperingatkan `cargo::redundant_homepage`, karena
  `homepage` di `Cargo.toml` sama dengan `repository`. Sebelum baris itu
  dihapus, pastikan field Homepage di `.deb`/`.rpm` tidak ikut hilang.
- **Hasil (25 Nov):**
  - `homepage` dihapus dari `Cargo.toml` (#33). Field Homepage di paket diambil
    dari `bundle.homepage` di `tauri.conf.json`.
  - Branch ✅ (25 Nov, atas persetujuan maintainer):
    - Remote: 19 branch yang sudah di-merge dihapus; lima branch Dependabot
      sudah terhapus sendiri.
    - Lokal: 18 branch yang sudah di-merge dihapus.
    - "Automatically delete head branches" kini menyala.

### D6 — Satu backlog yang hidup (P2, S)
- **Bukti:**
  - `docs/BACKEND-TODO.md` (9 Nov) masih mencatat Packaging dan First-run wizard
    sebagai belum.
  - Model mapping keymap masih tercatat P1, padahal §9 menolaknya dan editor
    keymap 0.1.9 sudah memenuhi kebutuhannya.
  - Akibatnya, agen dan kontributor yang membaca berkas itu bisa mengerjakan
    ulang hal yang sudah ada.
- **Kerjakan (BACKEND-TODO):** seperti F4. Centang yang sudah ada, pindahkan yang
  masih relevan ke To-do di sini, lalu arsipkan berkasnya ke `docs/archive/`.
- **Kerjakan (rencana ini):**
  - Pindahkan §3–§14, yang sudah selesai, ke
    `docs/archive/IMPROVEMENT-PLAN-2026-11.md` dengan nomor bagian yang sama,
    karena commit merujuk "§14", "G8", dan seterusnya.
  - Yang tersisa di sini hanya status, To-do, §9, §11, dan review terbaru.
- **DoD:**
  - Setiap item ⬜/⏳ hanya tercatat di satu tempat.
  - Tautan ke kedua berkas dari README, CONTRIBUTING, dan CHANGELOG masih
    jalan.
- **Hasil (25 Nov) ✅:**
  - §1–§14 dipindahkan ke `docs/archive/IMPROVEMENT-PLAN-2026-11.md`.
  - `BACKEND-TODO.md` dipindahkan ke `docs/archive/`; yang sudah ada dicentang.
  - Item yang masih terbuka masuk To-do dan Backlog di atas.

### D4 (lanjutan) — `lib.rs` tumbuh lagi (P3, M)
- **Bukti:**
  - 1875 baris setelah D4, 2143 di `main`, dan 2294 di branch ini.
  - Ada 55 `#[tauri::command]` di satu berkas itu.
  - #27 menambah 151 baris bersih, hampir semuanya untuk keymap dan Phantom
    (`find_phantom`, `apply_keymap`, stop daemon).
- **Kerjakan** bersama R24/R25: pindahkan command Phantom dan keymap ke
  `keymap.rs` dan `phantom_daemon.rs`, seperti yang dulu dilakukan dengan
  `logs.rs` dan `apk.rs`.
- **Hasil (25 Nov) ✅, #27:**
  - Command Phantom pindah ke `src-tauri/src/phantom.rs` dalam satu commit
    tanpa perubahan perilaku.
  - `lib.rs` di branch itu 2294 → 2031 baris; di `main` setelah semua merge
    2122 baris, karena #28 dan #34 menambah command Super key dan Private
    DNS.

### E6 — Block Ads lapis kedua: Private DNS opsional (P2, M)
- **Bukti:** 23 Nov, iklan masih sempat muncul dengan Block Ads aktif. File hosts
  hanya menangkap domain yang ada di daftar AdAway bawaan. SDK iklan yang
  memakai domain lain, DNS-over-HTTPS sendiri, atau IP langsung tetap lolos.
- **Ukur dulu:** catat domain iklan yang lolos, lewat logcat atau DNS, untuk
  2–3 game. Kalau cukup dengan menambah domain ke daftar bawaan, kerjakan itu
  saja.
- **Kalau tidak cukup:** switch terpisah di kartu Block Ads, "Block ads with
  AdGuard DNS".
  - Lewat adb, tanpa root: `settings put global private_dns_mode hostname` dan
    `private_dns_specifier dns.adguard-dns.com`.
  - Turn Off mengembalikan nilai sebelumnya.
- **Syarat privasi:**
  - CHANGELOG 0.1.9 menjanjikan "DNS still goes where it went", dan `PRIVACY.md`
    mendaftar setiap koneksi jaringan.
  - Opsi ini mengirim semua query DNS Android ke pihak ketiga. Jadi opsi ini
    harus opt-in, terpisah dari switch hosts, dan tertulis di `PRIVACY.md`
    sebelum dirilis.
- **DoD:**
  - Test untuk argv dan pemulihan nilai lama.
  - Live: tes iklan di game yang sama, iklan lolos sebelum dan bersih sesudah.
- **Hasil (25 Nov) ✅, #34:**
  - Switch "Also block with AdGuard DNS", mati secara default.
  - Nilai Private DNS lama disimpan sebelum ditulis (pola R26).
  - Server yang tidak dipasang ArchDroid tidak disentuh.
  - `PRIVACY.md`, README, dan daftar data sudah diperbarui.
  - **VM (Android 13, tanpa daftar hosts):** dengan perintah `settings` yang
    sama persis, `doubleclick.net`, `googleads.g.doubleclick.net`,
    `app-measurement.com`, dan `ads.pubmatic.com` berpindah dari IP asli ke
    `127.0.0.1`. `example.com` tetap normal. Berlaku dalam hitungan detik tanpa
    restart. Off menghapus kedua setting lagi.
  - Belum diuji: switch di app sungguhan, dan jaringan yang memblokir port
    853.

### E7 — Game kabur saat bergerak dan terasa lag (P2, S untuk ukur)
- **Bukti:**
  - 23 Nov, Clash Royale kabur saat layar digeser, tetapi screenshot di host
    tajam. Game juga terasa lag. Belum ada yang diukur.
  - Screenshot menangkap satu frame diam. Jadi kabur yang hanya muncul saat
    bergerak lebih mungkin berasal dari frame yang turun atau tersendat, sama
    dengan keluhan lag, daripada dari penskalaan.
- **Ukur dulu,** lewat adb, bukan rekaman layar desktop:
  - **Frame:** `dumpsys gfxinfo <paket> framestats` atau `dumpsys SurfaceFlinger
    --latency` selama 10 detik scroll, untuk FPS dan jank.
  - **Render:** `getprop ro.hardware.gralloc` dan `ro.hardware.egl`. Dari sini
    terlihat apakah Android memakai GPU host atau software rendering.
  - **Skala:** resolusi Android (`wm size`) dibanding ukuran jendela dan skala
    monitor di Hyprland. Ini dicek karena murah.
- **Kerjakan sesuai hasil:**
  - Software rendering: tambah cek di Host Diagnostics, lengkap dengan
    perbaikannya.
  - Skala tidak cocok: tambah preset Display yang mengikuti jendela atau
    monitor.
  - FPS rendah tanpa sebab di atas: catat sebagai batas Waydroid.
- **DoD:** angka sebelum dan sesudah dicatat di sini. Kode hanya diubah kalau
  angkanya menunjukkan sebab.
- **Hasil (25 Nov) ⏳, diukur statis di host ini tanpa menyalakan Waydroid:**
  - **Render:** `gralloc=gbm`, `egl=mesa` di `renderD129` (i915, Intel). Ini GPU,
    bukan software. NVIDIA (`renderD128`) tidak dipakai.
  - **Monitor:** DP-1 2560x1440@200 Hz (VRR aktif) dan eDP-2 1920x1200@180 Hz
    (VRR mati), keduanya dengan skala 1.25. Penskalaan bukan sebab utama,
    karena screenshot host (yang ikut terskala) tetap tajam.
  - **Hipotesis baru:** Android merender 60 fps. Di 200 Hz satu frame tampil
    3 atau 4 refresh bergantian (200/60 = 3,33), dan judder seperti itu
    terlihat kabur saat bergerak. Di eDP 180 Hz pas 3 refresh per frame.
  - **Alat:** `scripts/perf/game-frames.sh <paket> [detik]` mencetak jalur
    render (termasuk SwiftShader/llvmpipe), ukuran Android, monitor, lalu FPS,
    frame yang jatuh/jank, dan p50/p90/p99 jarak antar-frame dari `timestats`
    SurfaceFlinger. Diuji di VM: Settings saat di-scroll memberi 173 frame,
    60,2 fps, p50 16 ms, dan p99 33 ms, dengan renderer SwiftShader.
  - **Langkah pengguna:** buka Clash Royale, jalankan
    `scripts/perf/game-frames.sh com.supercell.clashroyale 20`, lalu geser layar
    selama 20 detik. Lakukan sekali di DP-1 dan sekali di eDP-2 (atau DP-1 di
    120/180 Hz), lalu bandingkan p90/p99 dan apakah kaburnya hilang.

### E8 — Host Diagnostics: GPU yang benar-benar dipakai Waydroid (P3, S)
- **Bukti:**
  - Cek `gpu` (`diagnostics.rs:107`) hanya bertanya apakah `/dev/dri/renderD128`
    atau `/dev/dri` ada.
  - Di host ini `renderD128` adalah NVIDIA. Waydroid sendiri memilih
    `renderD129` (Intel) di `waydroid_base.prop`, jadi lolosnya cek ini tidak
    membuktikan apa-apa.
- **Kerjakan:** tampilkan node `gralloc.gbm.device` dan driver kernelnya
  (`/sys/class/drm/<node>/device/driver`). Peringatkan bila tidak ada node yang
  bisa dipakai Mesa (hanya `nvidia`), karena Android lalu merender tanpa GPU.
- **DoD:** test tabel untuk i915, amdgpu, nvidia, dan node yang tidak ada;
  laporan host ini tetap lolos.

