notice-reason = { $reason }
cause-no-sources = Belum ada { -source-pl } yang diatur. Tambahkan satu di layar { -source-cap-pl }.
notice-not-aired = Episode { $episode } tayang { $when }.
notice-not-aired-yet = Episode { $episode } belum tayang. Episode { $next } tayang { $when }.
cause-airs-today = hari ini pukul { $time }
cause-airs-tomorrow = besok pukul { $time }
cause-airs-in-days = { $days ->
   *[other] dalam { $days } hari pukul { $time }
 }
cause-airs-on-date = pada { $date } pukul { $time }
cause-no-playable-stream = Tidak ada yang bisa diputar, meski { -source } itu sudah menjawab. Coba yang lain.
cause-title-no-match = Tidak bisa mencocokkan judul ini: { $detail }
cause-source-gone = Pilihan { -source } { $source } sudah tidak tersedia. Pilih yang lain.
cause-release-failed = Tidak bisa memulai { -release } itu: { $detail }
cause-sources-timed-out = Menunggu { -source-pl } terlalu lama. Coba lagi, atau pilih { -source } lain.
cause-no-source-has-episode = Tidak ada { -source } yang punya episode { $episode }: { $detail }
cause-source-cannot-download = Episode dari { -source } ini hanya bisa diputar, tidak bisa diunduh.
cause-no-source-could-list-show = Tidak ada { -source } yang bisa mendaftar judul ini, jadi tidak bisa diunduh.
cause-subtitles-merged = { $tracks ->
   *[other] { $tracks } trek subtitle digabungkan ke dalam video.
 }
cause-subtitles-left-apart = Subtitle tetap berupa file terpisah. { $reason }
cause-already-in-downloads = { $count ->
   *[other] { $count } episode sudah ada di unduhan.
 }
status-looking-up-title = Mencari judul…
status-loading-sources = Memuat { -source-pl }…
status-searching-sources = Mencari { -source-pl } untuk episode { $episode }…
notice-player-failed-to-start = Tidak bisa memulai mpv. Pastikan mpv bisa berjalan sendiri. ({ $reason })
notice-stream-no-video = Tidak ada video dari aliran itu. Mencoba { $next } dari { $total }.
notice-player-could-not-play = mpv tidak bisa memutar ini. { $reason }
notice-player-exited-silently = mpv ditutup tanpa memutar apa pun dan tanpa memberi alasan. Coba { -source } lain.
notice-player-not-playing = mpv belum mulai memutar setelah { $seconds } dtk. { -source } ini mungkin tidak punya data. Pastikan mpv bisa memutar file sendiri.
notice-no-player = Versi { -app-name } ini tidak bisa memutar video.
notice-mpv-missing = { $binary } tidak ditemukan. Pasang mpv untuk memutar episode.
notice-mpv-old = mpv { $version } bisa memutar, tetapi pemutar { -app-name } memerlukan mpv 0.36 atau yang lebih baru untuk tombol lewati, kartu { -up-next-cap }, menu, dan thumbnail.
notice-subtitles-unavailable = Subtitle dari { -source } ini tidak bisa dimuat. Pilih { -source } lain untuk mencoba lagi.
notice-opened-in-browser = Dibuka di peramban Anda.
notice-could-not-open = Tidak bisa membuka itu. { $reason }
notice-trailer-playing = Trailer sedang diputar di mpv.
notice-trailer-opened = Trailer dibuka di peramban Anda.
notice-trailer-failed = Tidak bisa membuka trailer. { $reason }
notice-filler-skipped = Episode pengisi dilewati. { -up-next-cap } adalah episode { $episode }.
notice-audio-now-subbed = Audio sekarang { -subbed }.
notice-audio-now-dubbed = Audio sekarang { -dubbed }.
notice-already-first-episode = Sudah di episode pertama.
notice-last-episode = Tidak ada episode setelah ini. Episode { $episode } adalah yang terakhir.
notice-no-next-episode = Tidak ada episode setelah ini.
notice-no-previous-episode = Tidak ada episode sebelumnya.
notice-match-reset-failed = Tidak bisa mengatur ulang kecocokan. { $reason }
notice-match-save-failed = Tidak bisa menyimpan kecocokan itu. { $reason }
notice-match-did-not-stick = Kecocokan itu tidak bertahan. { -source } mungkin sudah berubah. Coba pilih lagi.
notice-source-search-empty = Tidak ada hasil untuk “{ $query }”. Coba pencarian lain.
match-ruled-out-format = Format berbeda
match-ruled-out-episodes = Lebih banyak episode dari judul ini
match-ruled-out-year = Tahun berbeda
match-search-timed-out = Terlalu lama tidak menjawab.
match-search-source-off = Belum diaktifkan.
notice-source-pinned = { $source } akan dipakai untuk judul ini.
notice-source-automatic = Kembali ke pemilihan { -source } otomatis.
notice-source-choice-failed = Tidak bisa menyimpan pilihan itu. { $reason }
notice-no-source-could-list = Tidak ada { -source } yang bisa mendaftar judul ini: { $reason }
notice-episodes-failed = Tidak bisa memuat episode. { $reason }
notice-match-uncertain = { $source } menemukan lebih dari satu judul yang mungkin ini, jadi untuk sementara episodenya dari AniList.
notice-continue-hide-failed = Tidak bisa menghapus itu dari { -continue-cap }. { $reason }
notice-download-queued = Episode { $episode } dari { $title } masuk antrean.
notice-download-already-queued = Episode { $episode } dari { $title } sudah ada di unduhan.
notice-download-starting-now = Memulai episode { $episode } dari { $title } sekarang.
notice-download-cancelled = Unduhan dibatalkan.
notice-download-file-deleted = Unduhan dan file dihapus.
notice-download-removed-file-gone = Unduhan dihapus. File-nya sudah tidak ada.
notice-download-removed-file-kept = Unduhan dihapus, tetapi file-nya tidak bisa dihapus. { $reason }
notice-download-removed = Unduhan dihapus.
notice-downloads-cleared = { $count ->
   *[other] Menghapus { $count } unduhan yang sudah selesai.
 }
notice-download-finished = Episode { $episode } dari { $title } selesai diunduh.
notice-download-finished-note = Episode { $episode } dari { $title } selesai diunduh. { $note }
notice-download-failed = Tidak bisa mengunduh episode { $episode } dari { $title }. { $reason }
notice-downloads-none-queued = Tidak ada yang masuk antrean.
notice-downloads-none-queued-stopped = Tidak ada yang masuk antrean. { $reason }
notice-downloads-none-new = Tidak ada yang baru masuk antrean. { $existing }
notice-downloads-none-new-stopped = Tidak ada yang baru masuk antrean. { $existing } { $reason }
notice-episodes-queued-existing = { $count ->
   *[other] { $count } episode masuk antrean. { $existing }
 }
notice-episodes-queued-stopped = { $count ->
   *[other] { $count } episode masuk antrean, lalu berhenti. { $reason }
 }
notice-episodes-queued-existing-stopped = { $count ->
   *[other] { $count } episode masuk antrean, lalu berhenti. { $existing } { $reason }
 }
notice-keep-next-full = Ruang kosong tidak cukup untuk menyimpan episode setelahnya. Kosongkan ruang untuk mengunduh lebih banyak.
party-needs-server = Pilih server untuk { -watch-party } dulu, sebagai syncplay.server. { $server } dijalankan oleh { $operator }.
party-choose-server = Pilih server untuk { -watch-party } dulu. { $server } dijalankan oleh { $operator }.
notice-no-party = { -watch-party-cap-pl } tidak tersedia di versi { -app-name } ini.
party-needs-room = Untuk { -watch-party } diperlukan ruangan. Pilih satu dulu.
party-server-lost = Koneksi ke server { -watch-party } terputus. Mencoba lagi.
party-rejoined = Kembali ke { -watch-party }.
party-left = Keluar dari { -watch-party }.
