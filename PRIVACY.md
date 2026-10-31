# Kebijakan Privasi (Privacy Policy)

ArchDroid menghormati privasi pengguna. Kebijakan ini menjelaskan data yang disimpan secara lokal di mesin Anda dan komitmen privasi aplikasi.

---

## 1. Zero Telemetry & Phone-Home

- **Tanpa Pelacakan (Zero Analytics)**: ArchDroid tidak memuat library pelacakan, analytics, telemetry, ataupun identifier unik pengguna.
- **Tanpa Pengiriman Data**: Aplikasi tidak pernah mengirim data host, spesifikasi hardware, daftar aplikasi Android yang terpasang, atau statistik penggunaan ke server mana pun di internet.
- **Koneksi Jaringan**: ArchDroid hanya membuat dua jenis koneksi keluar:
  1. **Probe konektivitas (otomatis, tanpa data).** Untuk memeriksa apakah host online, ArchDroid membuka koneksi TCP ke `1.1.1.1:53` (Cloudflare) dan, hanya jika itu gagal, ke `8.8.8.8:53` (Google), dengan timeout 1 detik. Koneksi langsung ditutup tanpa mengirim byte apa pun; tidak ada query DNS, identifier, atau data host. Penyedia tersebut dan firewall Anda tetap dapat melihat bahwa alamat IP Anda membuka koneksi. Probe ini berjalan saat layar Setup atau Settings → Host Diagnostics dibuka, saat tombol scan ulang ditekan, saat membuat Diagnostic Bundle, dan sebelum init, installer GApps/Houdini, atau instalasi Phantom.
  2. **Unduhan yang diminta pengguna**, langsung ke sumber upstream:
     - Image resmi Android (SourceForge / Waydroid OTA server).
     - Dependensi paket distro melalui package manager host (`pacman`, `apt`, `dnf`, `zypper`).
     - Skrip injeksi GApps/Houdini (`waydroid_script`) yang dipin commit-nya dari GitHub upstream, paket Python ber-hash yang dibutuhkannya (PyPI), serta arsip GApps/libhoudini yang diunduh skrip itu dari sumber upstream-nya.
     - Source Phantom yang dipin commit-nya dari GitHub upstream; installer upstream-nya dapat mengunduh dependensi build sendiri.

---

## 2. Penyimpanan Data Lokal

Semua data konfigurasi, preferensi, dan runtime disimpan secara eksklusif di penyimpanan lokal mesin host:

| Jenis Data | Lokasi Penyimpanan | Tujuan |
|---|---|---|
| **Preferensi Aplikasi** | `~/.config/io.github.archdroid/` | Menyimpan profil tampilan, preset resolusi, DPI, dan konfigurasi keymapper Phantom. |
| **Marker & State Cache** | `~/.cache/archdroid/` | Menyimpan marker status task unprivileged dan cache file sementara. |
| **File Log Aplikasi** | `~/.local/share/io.github.wisangdg.archdroid/logs/` (lokasi persisnya tampil di Settings → Application Log) | Menyimpan log eksekusi proses lokal. Log ini hanya dibaca oleh pengguna dan tidak pernah diunggah secara otomatis. |
| **Diagnostic Bundle** | `~/Downloads/archdroid-diagnostics-*.txt` (atau `~`), mode `0600` | Dibuat **hanya** saat pengguna menekan Settings → Diagnostic Bundle → Create Bundle. Berisi versi OS/kernel/Waydroid, hasil diagnostik host, status add-on, dan sekitar 32 KiB terakhir log aplikasi serta log Waydroid. Log aplikasi mencatat argumen setiap command, sehingga bundle dapat memuat **nama paket Android dan nama file APK** yang pernah Anda pasang, jalankan, atau hapus. Path home dan nilai token/password disamarkan; nama paket dan nama file tidak. Dikumpulkan tanpa root dan tidak pernah diunggah; pengguna yang memutuskan untuk membagikannya. |
| **Data Sistem Waydroid** | `/var/lib/waydroid/` | File image Android sistem dan vendor milik container Waydroid (dikelola oleh root). |
| **Data Pengguna Android** | `~/.local/share/waydroid/` | Penyimpanan internal container Android, aplikasi APK yang terpasang, dan data aplikasi Android milik pengguna. |

---

## 3. Retensi & Penghapusan Data

Pengguna memegang kendali penuh atas data mereka. Seluruh data dapat dihapus kapan saja dengan menghapus direktori di atas atau mengikuti instruksi penghapusan bersih di [README.md](README.md#7-panduan-uninstall-bersih-clean-uninstall).
