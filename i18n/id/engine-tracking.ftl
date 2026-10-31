notice-signed-in = Berhasil masuk ke { $tracker }.
notice-signed-in-keychain = Berhasil masuk ke { $tracker }. Token disimpan di rantai kunci sistem.
notice-signed-in-file = Berhasil masuk ke { $tracker }. Rantai kunci tidak tersedia, jadi token disimpan dalam berkas pribadi di { -device } ini.
notice-signed-in-unsaved = Berhasil masuk ke { $tracker }, tetapi token tidak tersimpan, jadi Anda perlu masuk lagi nanti. { $reason }
notice-token-not-saved = Token masuk tidak dapat disimpan. Coba masuk lagi. { $reason }
notice-sign-in-failed = Tidak dapat masuk ke { $tracker }. Coba lagi. { $reason }
notice-sign-in-stopped = Proses masuk ke { $tracker } berhenti sebelum selesai. Coba lagi. { $reason }
notice-token-exchange-failed = Tidak dapat memperoleh token dari { $tracker }. Coba masuk lagi. { $reason }
notice-browser-not-opened = Tidak dapat membuka peramban untuk masuk. Pastikan ada peramban yang terpasang, lalu coba lagi. { $reason }
notice-sign-in-not-waiting = Tidak ada proses masuk ke { $tracker } yang menunggu itu. Mulai masuk lagi.
notice-sign-in-wrong-request = Masuk ke { $tracker } itu tidak kembali dari permintaan ini. Mulai masuk lagi.
notice-sign-in-no-token = Proses masuk ke { $tracker } kembali tanpa token. Coba lagi.
notice-mal-client-missing-build = Tidak dapat menyelesaikan masuk ke MyAnimeList: build ini tidak punya klien untuk itu.
notice-kitsu-signs-in-on-accounts = Kitsu masuk dari layar Akun.
notice-sign-in-unsupported = Masuk ke { $tracker } belum didukung.
notice-sign-in-needs-redirect = Masuk ke { $tracker } memerlukan pengalihan peramban, yang tidak dapat diterima aplikasi ini.
notice-anilist-implicit-flow = Metode masuk AniList ini hanya berfungsi di aplikasi { -app-name }. Untuk masuk di sini, atur { $setting } ke "loopback" dalam berkas konfigurasi.
notice-anilist-client-missing = AniList memerlukan ID dan rahasia klien. Atur { $client } dan { $secret } dalam berkas konfigurasi, lalu masuk lagi. Untuk mendapatkan alamat masuk, jalankan { -app-name } --login-url.
notice-anilist-paste-flow = Menempel kode tidak dapat dilakukan di sini. Keluar lalu jalankan { -app-name } --login.
notice-mal-client-missing = MyAnimeList memerlukan ID klien. Daftarkan aplikasi dengan jenis "other" di myanimelist.net/apiconfig, lalu atur { $setting } dalam berkas konfigurasi.
status-waiting-for-redirect = Menunggu { $tracker } mengalihkan kembali…
notice-mal-no-refresh-token = MyAnimeList tidak mengeluarkan refresh token. Anda perlu masuk lagi dalam sekitar satu bulan.
notice-client-id-missing = Atur { $setting } dalam berkas konfigurasi terlebih dahulu.
notice-signed-out = Keluar dari { $tracker }.
notice-signed-out-queue = { $count ->
   *[other] Keluar dari { $tracker }. { $count } perubahan dalam antrean disimpan untuk masuk lagi nanti.
 }
notice-sign-out-failed = Tidak dapat keluar dari { $tracker }. Coba lagi. { $reason }
notice-not-saved-because = Tidak dapat menyimpan pengaturan itu. { $cause }
notice-protection-saved = Perlindungan torrent disimpan.
notice-protection-not-checked = Tidak dapat memeriksa perlindungan torrent. { $cause }
notice-proxy-password-not-kept = Tidak dapat menyimpan kata sandi proxy. Masukkan lagi untuk mencoba menyimpannya. { $reason }
notice-show-setting-refused = Tidak dapat menyimpan pengaturan itu. { $value } bukan nilai untuk { $setting }.
notice-show-not-reset = Tidak dapat mengatur ulang pengaturan judul ini. { $reason }
notice-show-no-such-subtitles = Tidak dapat menyimpan pilihan subtitle itu. Judul ini belum pernah memakai subtitle tersebut.
notice-torrents-waiting-for-check = Torrent menunggu sampai pemeriksaan koneksi lolos lagi.
notice-mapping-data-refreshed = Data pemetaan diperbarui.
notice-mapping-data-failed = Tidak dapat memperbarui data pemetaan ({ $datasets }). Mencoba lagi dalam { $wait }. { $cause }
notice-episode-aired = { $title }, episode { $episode }, sudah tayang selagi Anda pergi.
notice-episodes-aired = { $count ->
   *[other] { $count } episode baru dari judul yang Anda tonton sudah tayang. Kalender memuatnya.
 }
notice-update-available = { $tag } sudah tersedia. Jalankan { -app-name } --update untuk memasangnya.
cause-confirm-no-protection = Konfirmasikan dulu bahwa Anda memang ingin tanpa perlindungan.
cause-confirm-warn-only = Konfirmasikan dulu bahwa Anda hanya ingin diperingatkan.
cause-proxy-needs-address = Proxy memerlukan host dan port.
cause-proxy-address-unusable = Alamat proxy tidak dapat diperiksa.
cause-server-unreachable = Server tidak dapat dihubungi.
