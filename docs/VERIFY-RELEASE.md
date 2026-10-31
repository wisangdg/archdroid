# Verifikasi Integritas dan Tanda Tangan Artefak Rilis ArchDroid

Setiap rilis resmi ArchDroid menyertakan file checksum `SHA256SUMS` dan tanda tangan digital GPG terpisah `SHA256SUMS.asc`.

## Kunci Publik Resmi Rilis
- **Pemilik Kunci:** `ArchDroid Release Signing Key <gwisang45@gmail.com>`
- **Key ID:** `18F47067ACC5BBF9`
- **Fingerprint:** `EB02 90C2 138F A472 E18F  0203 18F4 7067 ACC5 BBF9`
- **File Kunci:** [`packaging/release/archdroid-release.asc`](../packaging/release/archdroid-release.asc)

---

## Cara Verifikasi Manual

### 1. Impor Kunci Publik GPG
Unduh file `archdroid-release.asc` atau impor langsung:
```bash
gpg --import archdroid-release.asc
```

### 2. Verifikasi Tanda Tangan Digital Checksum
Pastikan file `SHA256SUMS` dan `SHA256SUMS.asc` berada di direktori yang sama dengan artefak yang diunduh:
```bash
gpg --verify SHA256SUMS.asc SHA256SUMS
```
Output yang valid akan menampilkan:
```text
gpg: Good signature from "ArchDroid Release Signing Key <gwisang45@gmail.com>"
Primary key fingerprint: EB02 90C2 138F A472 E18F  0203 18F4 7067 ACC5 BBF9
```

### 3. Verifikasi Integritas SHA-256 Artefak
Jalankan verifikasi hash:
```bash
sha256sum -c SHA256SUMS
```
Pastikan artefak yang ingin Anda gunakan bertanda `OK`, misalnya:
```text
ArchDroid_0.1.0_amd64.deb: OK
ArchDroid_0.1.0_amd64.AppImage: OK
archdroid-bin-0.1.0-1-x86_64.pkg.tar.zst: OK
ArchDroid-0.1.0-1.x86_64.rpm: OK
```

---

## Verifikasi Otomatis Menggunakan Skrip
Repo ini menyediakan skrip verifikasi mandiri menggunakan keyring terisolasi:
```bash
./scripts/release/verify-release.sh [path/to/downloaded-files]
```
atau via npm:
```bash
npm run release:verify
```
