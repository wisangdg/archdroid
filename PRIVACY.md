# Kebijakan Privasi (Privacy Policy)

ArchDroid menghormati privasi pengguna. Kebijakan ini menjelaskan data yang disimpan secara lokal di mesin Anda dan komitmen privasi aplikasi.

---

## 1. Zero Telemetry & Phone-Home

- **Tanpa Pelacakan (Zero Analytics)**: ArchDroid tidak memuat library pelacakan, analytics, telemetry, ataupun identifier unik pengguna.
- **Tanpa Koneksi Diam-Diam**: Aplikasi tidak pernah mengirim data host, spesifikasi hardware, daftar aplikasi Android yang terpasang, atau statistik penggunaan ke server mana pun di internet.
- **Koneksi Jaringan**: Satu-satunya aktivitas jaringan yang dilakukan adalah koneksi keluar langsung ke repositori resmi upstream saat pengguna secara eksplisit meminta:
  - Mengunduh image resmi Android (SourceForge / Waydroid OTA server).
  - Mengunduh dependensi paket distro melalui package manager host (`pacman`, `apt`, `dnf`, `zypper`).
  - Mengunduh skrip injeksi GApps/Houdini yang telah dipin commit-nya dari GitHub upstream.

---

## 2. Penyimpanan Data Lokal

Semua data konfigurasi, preferensi, dan runtime disimpan secara eksklusif di penyimpanan lokal mesin host:

| Jenis Data | Lokasi Penyimpanan | Tujuan |
|---|---|---|
| **Preferensi Aplikasi** | `~/.config/archdroid/` | Menyimpan profil tampilan, preset resolusi, DPI, dan konfigurasi keymapper Phantom. |
| **Marker & State Cache** | `~/.cache/archdroid/` | Menyimpan marker status task unprivileged dan cache file sementara. |
| **File Log Diagnostik** | `~/.cache/archdroid/logs/` (atau stdout terminal) | Menyimpan log eksekusi proses lokal. Log ini hanya dibaca oleh pengguna dan tidak pernah diunggah secara otomatis. |
| **Data Sistem Waydroid** | `/var/lib/waydroid/` | File image Android sistem dan vendor milik container Waydroid (dikelola oleh root). |
| **Data Pengguna Android** | `~/.local/share/waydroid/` | Penyimpanan internal container Android, aplikasi APK yang terpasang, dan data aplikasi Android milik pengguna. |

---

## 3. Retensi & Penghapusan Data

Pengguna memegang kendali penuh atas data mereka. Seluruh data dapat dihapus kapan saja dengan menghapus direktori di atas atau mengikuti instruksi penghapusan bersih di [README.md](README.md#7-panduan-uninstall-bersih-clean-uninstall).
