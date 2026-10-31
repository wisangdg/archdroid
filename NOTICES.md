# Pemberitahuan Pihak Ketiga (Third-Party Notices)

Dokumen ini mencantumkan atribusi, lisensi, dan pemberitahuan hak cipta dari perangkat lunak, pustaka, dan komponen pihak ketiga yang digunakan oleh atau diintegrasikan dengan ArchDroid.

---

## 1. Upstream Android Container: Waydroid

- **Proyek:** Waydroid
- **Situs Web:** [https://waydroid.net](https://waydroid.net)
- **Repositori:** [https://github.com/waydroid/waydroid](https://github.com/waydroid/waydroid)
- **Lisensi:** GNU General Public License v3.0 (GPL-3.0)
- **Pemberitahuan:**
  Waydroid adalah perangkat lunak terpisah yang bertindak sebagai runtime container Android pada kernel Linux host. ArchDroid berinteraksi dengan Waydroid melalui pemanggilan antarmuka baris perintah (`/usr/bin/waydroid`) dan komunikasi ADB standar. Waydroid tidak di-bundle atau dimodifikasi langsung di dalam kode sumber biner ArchDroid.

---

## 2. Helper Injeksi Add-on: waydroid_script

- **Proyek:** waydroid_script
- **Penulis:** casualsnek
- **Repositori:** [https://github.com/casualsnek/waydroid_script](https://github.com/casualsnek/waydroid_script)
- **Pinned Commit:** `48dbfaf34a6ddbe78688c530f9ba1c26522aafb2`
- **Lisensi:** GNU General Public License v3.0 (GPL-3.0)
- **Pemberitahuan & Alasan Lisensi:**
  `waydroid_script` tidak di-vendor atau didistribusikan di dalam biner maupun repositori ArchDroid. Skrip ini diunduh langsung dari repositori upstream saat pengguna secara eksplisit meminta instalasi add-on (GApps atau libhoudini) dan dijalankan di dalam direktori temporer privat terisolasi (`mode 0700`). Oleh karena itu, lisensi GPL-3.0 milik `waydroid_script` berdiri sendiri dan tidak mengubah lisensi MIT dari repositori ArchDroid itu sendiri.
- Rincian audit keamanan dan daftar jalur file yang dimodifikasi oleh skrip ini dapat dibaca pada [docs/THIRD-PARTY.md](docs/THIRD-PARTY.md).

---

## 3. Keymapper Virtual Touchscreen: Phantom

- **Proyek:** Phantom Keymapper
- **Penulis:** oliviermugishak
- **Repositori:** [https://github.com/oliviermugishak/phantom](https://github.com/oliviermugishak/phantom)
- **Pinned Commit:** `1a6b7edb423462c1bc546031ed86d9bf2cc1cffd`
- **Lisensi:** MIT License
- **Pemberitahuan:**
  Phantom di-clone dan dikompilasi secara lokal oleh pengguna pada tingkat unprivileged desktop (`$HOME/.local`). ArchDroid mengintegrasikan profil keymap dan peluncuran proses background Phantom tanpa memerlukan hak akses root.

---

## 4. Ikon Antarmuka (Icons): Lucide Icons

- **Proyek:** Lucide Icons (`lucide-react`)
- **Repositori:** [https://github.com/lucide-icons/lucide](https://github.com/lucide-icons/lucide)
- **Lisensi:** ISC License
- **Hak Cipta:** Copyright (c) for portions of Lucide are held by Cole Bemis 2013-2022 as part of Feather (MIT). All other copyright (c) 2022-present Lucide Contributors.

---

## 5. Tipografi & Font (Fonts)

- **Kebijakan Font:**
  ArchDroid **tidak mendistribusikan, membundle, atau memuat file font binary eksternal** melalui jaringan internet (sejalan dengan kebijakan keamanan Content Security Policy `font-src 'self'`).
- **Family Stack CSS:**
  Antarmuka merujuk pada font sistem lokal pengguna:
  - Teks umum: `'Outfit'`, `-apple-system`, `BlinkMacSystemFont`, `'Segoe UI'`, `Roboto`, `sans-serif`.
  - Teks monospace/terminal: `'JetBrains Mono'`, `monospace`.
  Jika font `'Outfit'` atau `'JetBrains Mono'` terpasang pada sistem operasi host pengguna, font tersebut digunakan; jika tidak, browser rendering engine akan beralih secara otomatis ke font sistem lokal yang setara (*system sans-serif / monospace*).

---

## 6. Framework & Pustaka Frontend / Backend

- **Tauri (`@tauri-apps/api`, `@tauri-apps/plugin-dialog`, crate `tauri`)**: Lisensi Apache-2.0 / MIT. Copyright (c) 2019 - present Tauri Programme within The Commons Conservancy.
- **React & React DOM**: Lisensi MIT. Copyright (c) Meta Platforms, Inc. and affiliates.
- **Vite**: Lisensi MIT. Copyright (c) 2019-present Evan You & Vite Contributors.
