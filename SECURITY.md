# Kebijakan Keamanan (Security Policy)

ArchDroid mengutamakan keamanan berlapis (*defense-in-depth*) dan batasan hak akses minimum (*least-privilege*) pada sistem operasi Linux.

---

## 1. Model Hak Akses & Privilege Boundary

### Proses Desktop & Webview (Unprivileged)
- **Non-Root Execution**: Aplikasi desktop ArchDroid berjalan sepenuhnya di level user biasa (*unprivileged desktop user*). ArchDroid **tidak boleh** dan **tidak pernah** dijalankan sebagai `root` langsung (`sudo archdroid`).
- **Tauri IPC & Capability Scoping**: Webview Tauri v2 diisolasi secara ketat. Tidak ada plugin shell bebas, plugin filesystem arbitrary, atau plugin HTTP yang diekspos ke frontend. Hanya perintah IPC terdaftar dan divalidasi yang dapat dipanggil oleh antarmuka.
- **Content Security Policy (CSP)**: Menggunakan CSP produksi yang memblokir eksekusi script remote, iframe eksternal, dan inline string injection (`default-src 'self'`). Font dan aset grafis dimuat 100% lokal dari bundle aplikasi (tanpa Google Fonts atau CDN pihak ketiga).

### Operasi Privileged (Root Elevation)
Operasi administratif sistem (seperti instalasi paket distro, inisialisasi `/var/lib/waydroid`, dan injeksi GApps/Houdini) menggunakan batasan keamanan ketat:
1. **Explicit Consent & Package Approval**:
   Sebelum otentikasi Polkit (`pkexec`) atau `sudo` dipicu, modal antarmuka menampilkan rincian perintah persis dan daftar paket yang akan dipasang. Backend memvalidasi ulang daftar persetujuan tersebut dan menolak eksekusi jika ada paket tambahan yang tidak disetujui.
2. **Isolasi Direktori Sementara**:
   Skrip root dieksekusi di dalam direktori temporer privat bertaraf `mode 0700` (`mktemp -d`).
3. **Sanitasi Environment**:
   Variabel sensitif seperti `SUDO_USER` dan env kustom dibersihkan. Cache root diarahkan ke direktori kerja privat (`$ARCHDROID_WORK/cache`) agar tidak mencemari kepemilikan file di direktori home user (`~/.cache`).
4. **Verifikasi Hash Dependensi Pihak Ketiga**:
   Skrip otomasi pendukung (seperti `waydroid_script`) mengunci seluruh paket dependensi Python menggunakan hash SHA-256 (`--require-hashes --only-binary=:all:`).

---

## 2. Versi yang Didukung

Pembaruan keamanan hanya disediakan untuk versi aktif pada branch `main` repositori resmi:

| Versi | Didukung |
|---|---|
| `0.1.x` (Latest) | :white_check_mark: |
| `< 0.1.0` | :x: |

---

## 3. Pelaporan Kerentanan (Reporting a Vulnerability)

Jika Anda menemukan celah atau kerentanan keamanan pada ArchDroid:
1. **Jangan buat isu publik di GitHub Issues.**
2. Laporkan secara privat melalui fitur [GitHub Security Advisories](https://github.com/wisangdg/archdroid/security/advisories/new) di repositori resmi, atau kirim email langsung ke:
   - **Email:** `gwisang45@gmail.com`
   - **Subjek:** `[SECURITY] Laporan Kerentanan ArchDroid`
3. Mohon sertakan:
   - Deskripsi langkah reproduksi (reproduction steps) atau proof of concept (PoC).
   - Versi ArchDroid dan distribusi Linux yang terpengaruh.
   - Analisis dampak keamanan.

Laporan yang diterima akan diverifikasi dalam waktu maksimal 3 hari kerja sebelum perbaikan dirilis secara terkoordinasi.
