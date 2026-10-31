notice-saved = { $label } disimpan.
notice-not-saved = Tidak dapat menyimpan pengaturan itu. { $reason }
notice-title-load-failed = Tidak dapat memuat judul ini. Periksa koneksi Anda, lalu coba lagi.
notice-playback-failed = Pemutaran gagal. Coba { -source } lain.
notice-engine-fault = Terjadi kesalahan di dalam { -app-name }. Aplikasi tetap berjalan, tetapi jika layar terlihat macet, keluar lalu buka lagi.
notice-database-set-aside = Basis data rusak, jadi { -app-name } membuat yang baru. Berkas lama disimpan di { $path }. Data yang disimpan { -tracker-pl } Anda akan kembali saat sinkronisasi selanjutnya.
start-failed-folder = { -app-name } tidak dapat membuat folder { $path }. Pastikan disk masih punya ruang dan folder itu dapat ditulisi, lalu coba lagi. { $reason }
start-failed-config = { $path } memiliki pengaturan yang tidak dapat digunakan { -app-name }: { $problem }. Perbaiki atau hapus pengaturan itu, lalu coba lagi.
config-reset-failed = { -app-name } tidak dapat memindahkan { $path } untuk mengatur ulang pengaturan. { $reason }
start-failed-database = { -app-name } tidak dapat membuka basis datanya di { $path }. Jika aplikasi { -app-name } lain sedang terbuka di folder ini, keluar dari aplikasi itu, lalu coba lagi. { $reason }
start-failed-network = { -app-name } tidak dapat menyiapkan koneksi jaringannya dari pengaturan jaringan di config.toml. { $reason }
start-failed = { -app-name } tidak dapat dimulai. Keluar, lalu buka lagi. { $reason }
start-failed-remote = Kendali jarak jauh tidak dapat dimulai. { $reason }
start-failed-player = Pemutar tidak dapat dimulai. { $reason }
notice-episodes-queued = { $count ->
   *[other] { $count } episode ditambahkan ke antrean unduhan.
 }
