# Strategi update, distribusi, dan kunci rilis

## Keputusan: update manual, tanpa updater di dalam aplikasi
ArchDroid **tidak** memiliki Tauri updater dan tidak mengunduh atau mengeksekusi
artefak apa pun sendiri. Alasan: aplikasi ini sudah meminta akses root; updater
menambah jalur eksekusi kode jarak jauh, dan CSP (`connect-src ipc:`) sengaja
memblokir semua koneksi jaringan dari WebView. Akibatnya **tidak ada update yang
mengeksekusi artefak tanpa signature**: pengguna selalu memasang paket secara
eksplisit.

### Cara upgrade
1. Arch: `pacman -U archdroid-bin-<versi>-x86_64.pkg.tar.zst`. AppImage: ganti file lama.
   `.deb`/`.rpm` adalah preview Tier 2 tanpa jaminan.
2. Verifikasi dulu sesuai [VERIFY-RELEASE.md](VERIFY-RELEASE.md)
   (`gpg --verify SHA256SUMS.asc SHA256SUMS`, `sha256sum -c SHA256SUMS`).
3. Cek versi terpasang di aplikasi/`archdroid --version`; bandingkan dengan halaman
   Releases: <https://github.com/wisangdg/archdroid/releases>.
4. Setiap rilis wajib punya release notes: perubahan, perubahan perilaku
   privileged/keamanan, dan langkah migrasi (config profil bermigrasi otomatis,
   lihat P1-7).

Rollback: pasang kembali paket versi sebelumnya yang sudah terverifikasi;
`profile.json` yang lebih baru tetap terbaca (field tak dikenal diabaikan).

## Kunci rilis GPG
| | |
|---|---|
| Key ID | `18F47067ACC5BBF9` (ed25519, sign/certify) |
| Fingerprint | `EB02 90C2 138F A472 E18F 0203 18F4 7067 ACC5 BBF9` |
| Dibuat | 2026-10-31 (epoch 1793447911) |
| Masa berlaku | **belum diset (tidak kedaluwarsa)** |
| Kunci publik | `packaging/release/archdroid-release.asc` |
| Kunci privat | hanya di mesin pemilik/offline; **tidak pernah ada di repo** (dijaga oleh `src/lib/release-hygiene.test.js`) |

### Tindakan yang harus dilakukan pemilik (tidak bisa dilakukan dari repo)
- [ ] Set masa berlaku 2 tahun: `gpg --edit-key 18F47067ACC5BBF9` → `expire`, lalu terbitkan ulang `.asc`.
- [ ] Simpan backup terenkripsi kunci privat + sertifikat revokasi di media offline
      terpisah, lalu catat lokasinya di sini: `Lokasi backup: ______`.
- [ ] Publikasikan fingerprint di kanal kedua selain repo: deskripsi halaman
      GitHub Releases/profil pemilik, dan kunci di keyserver (`keys.openpgp.org`).
      Fingerprint yang hanya ada di repo tidak melindungi bila repo dibobol.
      Sebagian: fingerprint sudah ada di release notes
      [v0.1.1](https://github.com/wisangdg/archdroid/releases/tag/v0.1.1);
      profil pemilik dan keyserver belum.

### Rotasi
1. Buat kunci baru, tandatangani pengumuman rotasi dengan kunci lama.
2. Publikasikan fingerprint baru di semua kanal, ganti `.asc`, `VERIFY-RELEASE.md`,
   dan `GPG_KEY_ID` default di `scripts/release/*.sh`.
3. Rilis berikutnya ditandatangani kunci baru; kunci lama dibiarkan sampai kedaluwarsa.

### Revokasi (kunci bocor/hilang)
1. `gpg --import revoke.asc`, unggah ke keyserver, ganti `.asc` di repo dengan versi revoked.
2. Umumkan di Releases dan kanal kedua; tandai semua rilis lama sejak kebocoran sebagai
   tidak tepercaya dan terbitkan ulang dengan kunci baru.
