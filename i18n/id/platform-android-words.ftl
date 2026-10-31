episode-short-dubbed = E{ $episode } · { -dubbed-cap }
episode-long-dubbed = Episode { $episode } · { -dubbed-cap }
downloads-live-paused = { $count ->
   *[other] { $count } episode dijeda
 }
downloads-live-lead = { $title } E{ $episode }
downloads-live-left = { $size } tersisa
downloads-live-progress = { $done } dari { $total }
downloads-live-paused-one = Dijeda
downloads-live-waiting = Menunggu
settings-seed-ratio-until = Sampai rasio { $ratio }
settings-upload-rate = { $size }/s
settings-coming-up-dubbed = { $count ->
   *[other] { $names } memberi tahu saat dub-nya keluar.
 }
settings-coming-up-switched-off = { $count ->
   *[other] { $names } dimatikan di pengaturannya sendiri.
 }
settings-coming-up-opens = Tayangan membuka pengaturannya.
torrents-check-note-warn = { -app-name } memeriksa dari mana lalu lintas torrent keluar saat torrent mulai dan setiap menit selama berjalan, dan memberi peringatan jika pemeriksaan itu gagal. Yang melindungi Anda adalah VPN atau proxy Anda.
torrents-check-note-stop = { -app-name } memeriksa dari mana lalu lintas torrent keluar saat torrent mulai dan setiap menit selama berjalan, dan menghentikan torrent jika pemeriksaan itu gagal. Yang melindungi Anda adalah VPN atau proxy Anda.
torrents-protect-proxy-name = Proxy SOCKS5
torrents-protect-vpn-name = VPN di { -device } ini
torrents-warn-body = { -app-name } akan memberi peringatan, bukan menjeda. Jika VPN atau proxy Anda mati, orang lain bisa melihat alamat IP Anda.
torrents-proxy-as = sebagai { $user }
title-facts-aired = Ditayangkan { $dates }
title-facts-airing = Tayang sejak { $date }
title-facts-airs = Tayang mulai { $date }
media-dates-range = { $from } sampai { $to }
title-also-on-more = { $more ->
   *[other] { $first } dan { $more } lainnya
 }
title-adult-mark = 18+
downloads-storage-used-free = { $used } terpakai · { $free } tersedia
downloads-storage-used = { $used } terpakai
downloads-time-left = { $time } tersisa
downloads-rate = { $size }/s
downloads-uploaded = { $size } diunggah
downloads-ratio-of = { $ratio } dari { $goal }
downloads-group-summary = { $count ->
   *[other] { $count } episode · { $size }
 }
episode-menu-download = Unduh
episode-menu-download-paused = Unduhan dijeda
reason-timeout-anilist = AniList terlalu lama menjawab.
reason-timeout-server = Server terlalu lama menjawab.
reason-busy-anilist = AniList sedang sibuk. Coba lagi dalam semenit.
reason-busy-server = Server sedang sibuk. Coba lagi dalam semenit.
reason-problem-anilist = AniList mengalami masalah.
reason-problem-server = Server mengalami masalah.
reason-silent-anilist = AniList tidak merespons.
reason-silent-server = Server tidak merespons.
updates-installing-plain = Memasang
updates-closes-as-installs = { -app-name } ditutup saat dipasang
updates-open-release = Buka halaman
country-korea = Korea Selatan
search-genres-picked = { $count ->
   *[other] { $count } genre
 }
search-formats-picked = { $count ->
   *[other] { $count } format
 }
search-statuses-picked = { $count ->
   *[other] { $count } status
 }
filter-score-any = Semua
filter-years-from = Sejak { $year }
filter-years-between = { $from } sampai { $to }
filter-tag-rank-title = Peringkat tag minimal
filter-tag = Tag
search-filter-by = Saring berdasarkan { $name }
search-loading-genres = Memuat genre
search-paused-rate-limit = Pencarian saat mengetik dijeda sebentar, agar tetap dalam batas AniList. Tekan tombol cari untuk mencari sekarang.
action-clear-all = Hapus semua
filter-source-material = Berdasarkan
search-detail-episode = { $status } · E{ $episode }
sources-use = Pakai { $name }
sources-move = Pindahkan { $name }
sources-reorder = Atur ulang
sources-version-available = { $version } tersedia
sources-update-from = Versi { $version } dari { $store }.
sources-detail-off = Nonaktif. Posisinya tetap, dan tidak dicoba sampai diaktifkan lagi.
sources-fallback-title = { -source-cap }
reason-source-timeout = { -source-cap } itu terlalu lama menjawab.
reason-source-busy = { -source-cap } sedang sibuk. Coba lagi dalam semenit.
reason-source-problem = { -source-cap } mengalami masalah.
reason-source-turned-away = { -source-cap } menolak permintaan.
reason-source-no-address = Alamatnya tidak ditemukan.
reason-source-unreachable = Tidak dapat dihubungi.
stores-updates-waiting = { $count ->
   *[other] { $count } pembaruan menunggu
 }
stores-checked-ago = diperiksa { $ago }
stores-preview-key-link = Kunci cocok dengan tautan
stores-preview-key-invite = Kunci cocok dengan undangan
stores-key-verified = Terverifikasi
stores-fallback-title = { -plugin-store-cap }
stores-fallback-plugin = { -plugin-cap }
stores-fallback-name = { -plugin-store }
stores-link-sheet-detail = Buka tautan ini di { -device } Anda yang lain untuk bergabung ke { -plugin-store } di sana. Tautan ini hanya bisa dipakai sekali.
accounts-synced-ago = Disinkronkan { $ago }
accounts-card-line = { $tracker } · { $status }
accounts-card-description = { $name }, { $line }. Membuka Akun
accounts-card-description-signed-out = Belum masuk. Membuka Akun
accounts-signed-in-plain = Sudah masuk
settings-summary-notify-on = Episode baru aktif
settings-summary-notify-off = Episode baru nonaktif
settings-summary-sources = { $count ->
   *[other] { $count } { -source }
 }
settings-summary-no-sources = Belum ada { -source-pl }
settings-summary-torrents-on = torrent aktif
settings-summary-torrents-off = torrent nonaktif
library-storage-on-device = { $size } di { -device } ini
library-storage-none = Tidak ada di { -device } ini
library-detail-of = E{ $next } dari { $total }
library-detail-next = E{ $next }
library-sort-last-watched = Terakhir ditonton
library-sort-current = Dipakai
settings-notify-permission-note = { -app-name } meminta izin saat Anda pertama kali menyalakan ini.
settings-notify-refused = Nonaktif: Android tidak mengizinkan notifikasi { -app-name }
settings-notify-system-title = Pengaturan notifikasi
settings-notify-system-detail = Suara, getaran, dan layar kunci
settings-titles-line-signed-out = Bahasa Inggris saat belum masuk ke AniList
settings-titles-line-account-language = { $language }, dari akun AniList Anda
settings-titles-line-follows = Mengikuti akun AniList Anda
settings-titles-line-english = Romaji jika tidak ada judul Inggris
settings-titles-line-romaji = Judul Jepang dalam huruf Latin
settings-titles-note-account = Mengikuti akun AniList Anda, yang diatur ke { $language }. Saat Anda belum masuk ke AniList, judul tampil dalam bahasa Inggris.
settings-titles-note-follows = Mengikuti akun AniList Anda. Saat Anda belum masuk ke AniList, judul tampil dalam bahasa Inggris.
settings-titles-note-english-missing = { $title } tidak punya judul Inggris di AniList, jadi tetap memakai judul romaji-nya.
settings-titles-note-english = Judul Inggris sebagaimana tercantum di AniList, dan romaji jika tidak ada.
settings-titles-note-romaji = Judul Jepang dalam huruf Latin, sebagaimana tercantum di AniList.
settings-index-none = Belum diatur
settings-stores-with-updates = { $stores } · { $updates }
settings-seeding-best-effort = Sebisa mungkin di { -device } ini: seeding hanya berjalan selama { -app-name } terbuka.
settings-downloads-count = { $count ->
   *[other] { $count } episode
 }
settings-anilist-footer = Kemajuan dan perubahan daftar disinkronkan ke AniList. { -library-cap } menampilkan daftar AniList Anda.
accounts-sign-in-again-button = Masuk lagi
accounts-review-differences = { $count ->
   *[other] Tinjau { $count } perbedaan
 }
settings-licence-title = Lisensi
settings-licence-value = MIT, open source dan nonkomersial
settings-typefaces-value = Roboto Flex dan Zen Kaku Gothic New, di bawah SIL Open Font License 1.1
settings-libraries-value = JNA dan AndroidX, di bawah Apache License 2.0
settings-report-bug-value = Membuka GitHub di peramban Anda
downloads-storage-share-description = Unduhan memakan { $size }
settings-aside-notifications = Satu notifikasi saat episode dari tayangan yang Anda ikuti keluar: nama tayangan dan nomor episodenya, tidak pernah judul episodenya. Sebuah tayangan bisa mematikan notifikasinya sendiri di pengaturannya.
settings-aside-downloads = Unduhan disimpan di ruang milik { -app-name } pada { -device } ini, yang tidak bisa dibaca aplikasi lain. Menghapus aplikasi juga menghapusnya.
settings-aside-connection-check = Pemeriksaan koneksi
settings-aside-sources = { -source-cap-pl } dicoba berurutan, dari atas ke bawah. Sebuah tayangan bisa memakai { -source } sendiri, yang dipilih dari halaman judulnya.
settings-upscaling-description = Gambar yang sama, setengah kiri sebagai 480p asli, setengah kanan dengan { $mode }
settings-upscaling-source = 480p asli
settings-upscaling-note-off-original = 480p asli, diskalakan agar pas tanpa pemrosesan.
torrents-sheet-turn-on-title = Aktifkan torrent
torrents-sheet-protect-title = Lindungi lalu lintas torrent
torrents-sheet-turn-on-button = Aktifkan
torrents-sheet-host = Host
torrents-sheet-port = Port
torrents-sheet-proxy-auth-note = Isi nama pengguna dan kata sandi hanya jika proxy memintanya.
torrents-sheet-edit-proxy-title = Ubah proxy
title-facts-also-on = Juga di
title-fetch-failed-detail = AniList tidak merespons.
continue-resize-queue = Ubah ukuran antrean
continue-go-to-title = Buka judul
continue-short-left = { $episode } · { $left }
continue-airing-out = E{ $episode } · { $time } · sudah keluar { $ago }
continue-airing-in = E{ $episode } · { $time } · dalam { $countdown }
calendar-line-out = { $episode } sudah keluar { $ago }
calendar-line-in = { $episode } dalam { $countdown }
nav-accounts-and-settings-user = { $user }, akun dan pengaturan
episode-unaired-label = Episode { $number }, { $title }. Belum keluar
episode-unaired-label-plain = Episode { $number }. Belum keluar
episode-unaired-airs-label = Episode { $number }, { $title }. Tayang { $airs }, belum keluar
episode-unaired-airs-label-plain = Episode { $number }. Tayang { $airs }, belum keluar
tv-sheet-on-network = Di jaringan ini
tv-sheet-not-listed = Tidak muncul di daftar
tv-sheet-scan = Pindai kode di TV
tv-sheet-code-on = Kode di { $name }
tv-sheet-pairing-button = Memasangkan
tv-sheet-connecting = Menghubungkan ke TV
tv-sheet-type-code = Ketik kodenya
tv-sheet-volume = Volume TV
tv-sheet-volume-down = Kecilkan volume
tv-sheet-volume-up = Besarkan volume
tv-sheet-volume-value = { $value } dari 100
tv-sheet-nothing-playing = Tidak ada yang diputar
tv-sheet-position = Posisi di TV
tv-sheet-position-state = { $at } dari { $total }
tv-sheet-request-titled = { $title }, episode { $episode }
tv-sheet-request-plain = Episode { $episode }
tv-sheet-footer = TV mencantumkan { -device } ini dengan kode { $code }. Jika Anda melupakan TV di sini, TV juga akan melupakan { -device } ini.
tv-sheet-title-fallback = TV
episode-line-detail = E{ $number } · { $detail }
calendar-day-label = { $day }
calendar-day-label-today = { $day }, hari ini
calendar-day-label-airing = { $day }, ada episode tayang
calendar-day-label-today-airing = { $day }, hari ini, ada episode tayang
continue-show-airing-in-calendar = Tampilkan yang tayang hari ini di Kalender
continue-show-season-in-seasonal = Tampilkan musim ini di Musiman
search-searching-filters = Mencari { $filters }
episode-number-plain = Episode { $number }
episode-number-title-text = Episode { $number } · { $title }
player-position-state = { $at } dari { $total }
player-sheet-finding = Mencari { -source-pl }…
player-sheet-none = Tidak ada { -source-pl } untuk episode ini.
player-sheet-episodes-loading = Daftar episode sedang dimuat.
player-dual-audio = { -subbed-cap } dan { -dubbed }
notif-new-episodes = { $count ->
   *[other] { $count } episode baru
 }
notif-hidden-text = Episode baru
notif-episode-out = Episode { $episode } sudah keluar.
notif-episode-out-dubbed = Episode { $episode } sudah keluar, { -dubbed }.
notif-episode-out-next = Episode { $episode } sudah keluar. Lanjut ke episode { $next } untuk Anda.
notif-episode-out-dubbed-next = Episode { $episode } sudah keluar, { -dubbed }. Lanjut ke episode { $next } untuk Anda.
device-name-phone = Ponsel Android
device-name-tablet = Tablet Android
tv-session-starting = Memulai di { $tv }
tv-session-line = { $line } · { $tv }
widget-line-left = { $episode } · { $minutes } menit lagi
widget-line-of = E{ $episode } dari { $total }
widget-continue-title = { -continue-cap }
widget-caught-up-title = Sudah mengikuti semua
widget-caught-up = Anda sudah mengikuti semua.
widget-caught-up-next = { $title } E{ $episode } tayang { $airs }.
widget-day-date = { $day }
widget-play-dubbed-label = Putar episode { $episode }, { -dubbed }, dari { $title }
tv-action-resume = Lanjutkan episode { $episode }
tv-action-resume-dubbed = Lanjutkan episode { $episode } · { -dubbed-cap }
tv-action-out = Putar episode { $episode } (keluar { $day })
tv-action-not-out = Putar episode { $episode } (belum keluar)
tv-out-today = hari ini
tv-hero-resume = Lanjutkan { $episode }
tv-hero-start = Putar { $episode }
