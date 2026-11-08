# Backup data app — catatan desain (E5)

Status: **belum diimplementasikan; butuh keputusan maintainer.** Fakta di bawah
diperiksa live pada 9 Nov 2026 (Waydroid 1.6.3, LineageOS 20 `userdebug`,
Arch, host uid 1000).

## Fakta

| Pertanyaan | Hasil |
|---|---|
| `adb shell` sebagai apa? | `uid=2000(shell)`, bukan root. |
| `adb root`? | "not available in Waydroid" (`ro.debuggable=0`). |
| Data app di Android | `/data/data/<pkg>` mode `700`, pemilik uid app (mis. `u0_a148`). |
| Data app di host | `~/.local/share/waydroid/data/data/<pkg>`, pemilik uid **10148** (tanpa user namespace), mode `700`: user desktop **tidak** bisa membacanya. Direktori induk `data/data` milik user (771). |
| `run-as <pkg>` | Hanya untuk app `debuggable`; tidak berlaku untuk app biasa. |
| `tar` di Android | Ada (`/system/bin/tar`). |
| APK | Bisa diambil tanpa root: `adb shell pm path <pkg>` + `adb pull` (diuji: 4 split ARCore). |

Kesimpulan: **APK bisa dicadangkan tanpa root, data app tidak.** Data butuh
root, lewat `waydroid shell` (root di container) atau dengan membaca direktori
host sebagai root.

## Pilihan

1. **APK saja, tanpa root (S).** Simpan semua split ke satu `.apks` di
   `~/Downloads`. Bisa dipasang ulang dengan fitur split APK (E4). Data dan login
   app hilang. Jujur dan aman, tetapi bukan "backup" dalam arti penuh.
2. **APK + data lewat root (L).** Script privileged menjalankan
   `tar -C ~/.local/share/waydroid/data/data -cf - <pkg>` (dengan `--numeric-owner`)
   lalu menulis arsip milik user (`0600`). Restore: pasang `.apks`, hentikan app
   (`am force-stop`), ekstrak sebagai root, lalu `chown -R` ke uid baru app
   (diambil dari `stat` direktori yang dibuat oleh install) dan `restorecon`
   bila SELinux enforcing. Butuh prompt root untuk backup dan restore.
3. **Seluruh `/var/lib/waydroid` + `~/.local/share/waydroid` (L).** Snapshot
   container penuh sebelum Factory Reset (root, beberapa GB). Restore = kembalikan
   kedua direktori dengan sesi mati. Paling sederhana dan paling andal untuk
   kasus "sebelum wipe", tetapi besar dan semua-atau-tidak.

## Risiko yang harus diputuskan

- Prompt root tambahan untuk operasi yang terasa "baca saja" (backup).
- Restore data ke versi app yang berbeda bisa membuat app crash; perlu cek
  `versionCode` dan menolak downgrade.
- Arsip berisi token login dan data pribadi: mode `0600`, jangan pernah masuk
  Diagnostic Bundle, dan sebutkan di `PRIVACY.md`.
- uid app berubah setelah reinstall atau wipe, jadi restore tanpa `chown` gagal
  diam-diam (app melihat direktori yang bukan miliknya).

## Rekomendasi

Mulai dari **opsi 1** (tanpa root, kecil, langsung berguna bersama E4), lalu
**opsi 3** di dialog Factory Reset bila pengguna memintanya. Opsi 2 hanya bila
ada permintaan nyata untuk backup per app dengan data, karena restore-nya yang
paling rapuh.
