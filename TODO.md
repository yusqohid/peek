# TODO — peek

Roadmap pengembangan berikut disusun berdasarkan kondisi proyek saat ini.
Prioritaskan setiap fase secara berurutan agar aplikasi selalu dapat dibangun
dan diuji.

## 1. Stabilitas dan quality gate

- [x] Jalankan `cargo fmt --check`, `cargo test`, dan `cargo clippy -- -D warnings` sebelum setiap commit.
- [x] Perbaiki test TODO scanner yang assertion-nya selalu benar (`usize >= 0`) menjadi assertion yang bermakna.
- [x] Tambahkan workflow CI GitHub Actions untuk format, test, dan Clippy pada setiap pull request dan push.
- [x] Pisahkan perubahan visual/UI ke commit kecil yang mudah ditinjau dan direvert.

## 2. Cakupan pengujian

- [x] Buat fixture sementara untuk pengujian scanner; jangan hanya menguji repository proyek ini sendiri.
- [x] Uji deteksi semua marker proyek, kedalaman scan, `exclude_dirs`, dan `ignored_projects`.
- [x] Uji urutan proyek, sort order, toggle proyek ignored, dan batas `selected_project`.
- [x] Uji TODO/FIXME/HACK/BUG scanner terhadap contoh yang valid, file besar, dan direktori yang dikecualikan.
- [x] Uji GitHub client dengan mock HTTP: sukses, user tidak ditemukan, rate limit, dan respons tidak lengkap.

## 3. Correctness dan konsistensi data

- [x] Clamp/reset `selected_project` setelah daftar visible berubah, terutama sesudah toggle ignored projects.
- [x] Perbaiki penentuan `last_modified` untuk marker glob seperti `*.csproj`; simpan path marker yang benar, bukan pola glob.
- [ ] Tentukan kontrak TODO scanner: marker teks sederhana atau hanya komentar. Jika hanya komentar, gunakan pendekatan language-aware.
- [ ] Selaraskan statistik `total_bytes` dengan filter bahasa pemrograman yang dipakai untuk LOC.

## 4. Performa dan responsivitas TUI

- [x] Pindahkan pemindaian workspace, tokei, analisis Git, TODO scan, dan fetch GitHub dari thread UI.
- [x] Tambahkan worker/background task dan channel untuk progress, hasil parsial, error, serta pembatalan refresh.
- [x] Tampilkan progress nyata per proyek ketika scan/refresh berjalan.
- [x] Cache hasil analisis per proyek berdasarkan path, timestamp marker, dan/atau Git HEAD agar refresh cepat pada workspace besar.
- [x] Terapkan `refresh_interval_secs` yang saat ini baru tersimpan di konfigurasi.

## 5. UX dan produk

- [x] Tambahkan empty state dan error state yang jelas untuk scan directory yang tidak ada atau tidak dapat diakses.
- [x] Tambahkan pencarian/filter proyek dan opsi sort ascending/descending.
- [x] Pastikan layout tetap berguna pada terminal sempit; tampilkan compact view bila perlu.
- [x] Jika memakai Nerd Font icon, gunakan escape Rust yang valid seperti `\u{f073}` dan sediakan fallback emoji/teks.
- [ ] Dokumentasikan kebutuhan terminal/font, konfigurasi GitHub, dan batasan data GitHub public di README.

## 6. Rilis

- [ ] Tambahkan changelog dan kebijakan versi semantik.
- [ ] Tambahkan release build CI untuk platform utama.
- [ ] Verifikasi binary rilis dengan workspace kecil dan besar sebelum publikasi.
