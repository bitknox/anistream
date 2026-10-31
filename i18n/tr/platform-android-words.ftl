episode-short-dubbed = E{ $episode } · { -dubbed-cap }
episode-long-dubbed = Bölüm { $episode } · { -dubbed-cap }
downloads-live-paused = { $count ->
    [one] Bir bölüm duraklatıldı
   *[other] { $count } bölüm duraklatıldı
 }
downloads-live-lead = { $title } E{ $episode }
downloads-live-left = { $size } kaldı
downloads-live-progress = { $done } / { $total }
downloads-live-paused-one = Duraklatıldı
downloads-live-waiting = Sırada bekliyor
settings-seed-ratio-until = Oran { $ratio } olana kadar
settings-upload-rate = { $size }/sn
settings-coming-up-dubbed = { $count ->
    [one] { $names } dublajı çıktığında bildirir.
   *[other] { $names } dublajları çıktığında bildirir.
 }
settings-coming-up-switched-off = { $count ->
    [one] { $names } kendi ayarında kapalı.
   *[other] { $names } kendi ayarlarında kapalı.
 }
settings-coming-up-opens = Bir gösteri ayarlarını açar.
torrents-check-note-warn = { -app-name }, bir torrent başladığında ve çalıştığı sürece her dakika torrent trafiğinin nereden çıktığını denetler. Denetim başarısız olursa sizi uyarır. Sizi koruyan şey VPN'iniz ya da proxy'nizdir.
torrents-check-note-stop = { -app-name }, bir torrent başladığında ve çalıştığı sürece her dakika torrent trafiğinin nereden çıktığını denetler. Denetim başarısız olursa torrent'leri durdurur. Sizi koruyan şey VPN'iniz ya da proxy'nizdir.
torrents-protect-proxy-name = SOCKS5 proxy
torrents-protect-vpn-name = Bu { -device }da VPN
torrents-warn-body = { -app-name } torrent'leri duraklatmak yerine sizi uyarır. VPN'iniz ya da proxy'niz çalışmıyorsa başkaları IP adresinizi görebilir.
torrents-proxy-as = { $user } olarak
title-facts-aired = Yayınlandı { $dates }
title-facts-airing = { $date } tarihinden beri yayında
title-facts-airs = { $date } tarihinde başlayacak
media-dates-range = { $from } ile { $to } arası
title-also-on-more = { $more ->
    [one] { $first } ve bir tane daha
   *[other] { $first } ve { $more } tane daha
 }
title-adult-mark = 18+
downloads-storage-used-free = { $used } kullanıldı · { $free } boş
downloads-storage-used = { $used } kullanıldı
downloads-time-left = { $time } kaldı
downloads-rate = { $size }/sn
downloads-uploaded = { $size } gönderildi
downloads-ratio-of = { $ratio } / { $goal }
downloads-group-summary = { $count ->
    [one] Bir bölüm · { $size }
   *[other] { $count } bölüm · { $size }
 }
episode-menu-download = İndir
episode-menu-download-paused = İndirme duraklatıldı
reason-timeout-anilist = AniList yanıt vermesi çok uzun sürdü.
reason-timeout-server = Sunucunun yanıtı çok uzun sürdü.
reason-busy-anilist = AniList meşgul. Bir dakika sonra yeniden deneyin.
reason-busy-server = Sunucu meşgul. Bir dakika sonra yeniden deneyin.
reason-problem-anilist = AniList bir sorunla karşılaştı.
reason-problem-server = Sunucu bir sorunla karşılaştı.
reason-silent-anilist = AniList yanıt vermedi.
reason-silent-server = Sunucu yanıt vermedi.
updates-installing-plain = Kuruluyor
updates-closes-as-installs = { -app-name } kurulurken kapanır
updates-open-release = Sayfayı aç
country-korea = Güney Kore
search-genres-picked = { $count ->
    [one] Bir tür
   *[other] { $count } tür
 }
search-formats-picked = { $count ->
    [one] Bir biçim
   *[other] { $count } biçim
 }
search-statuses-picked = { $count ->
    [one] Bir durum
   *[other] { $count } durum
 }
filter-score-any = Herhangi
filter-years-from = { $year } ve sonrası
filter-years-between = { $from } ile { $to } arası
filter-tag-rank-title = Etiket derecesi en az
filter-tag = Etiket
search-filter-by = Şuna göre filtrele: { $name }
search-loading-genres = Türler yükleniyor
search-paused-rate-limit = Yazarken arama, AniList'in sınırları içinde kalmak için bir süreliğine duraklatıldı. Şimdi aramak için arama tuşuna basın.
action-clear-all = Tümünü temizle
filter-source-material = Temel alınan
search-detail-episode = { $status } · E{ $episode }
sources-use = { $name } kullan
sources-move = { $name } taşı
sources-reorder = Yeniden sırala
sources-version-available = { $version } mevcut
sources-update-from = { -release-cap } { $version }, { $store } üzerinden.
sources-detail-off = Kapalı. Yerini korur ve yeniden açılana kadar denenmez.
sources-fallback-title = { -source-cap }
reason-source-timeout = Yanıt vermesi çok uzun sürdü.
reason-source-busy = Meşgul. Bir dakika sonra yeniden deneyin.
reason-source-problem = Bir sorunla karşılaştı.
reason-source-turned-away = İsteği geri çevirdi.
reason-source-no-address = Adresi bulunamadı.
reason-source-unreachable = Ulaşılamadı.
stores-updates-waiting = { $count ->
    [one] Bir güncelleme
   *[other] { $count } güncelleme
 }
stores-checked-ago = Denetlendi: { $ago }
stores-preview-key-link = Anahtar bağlantıyla eşleşiyor
stores-preview-key-invite = Anahtar davetle eşleşiyor
stores-key-verified = Doğrulandı
stores-fallback-title = { -plugin-store-cap }
stores-fallback-plugin = { -plugin-cap }
stores-fallback-name = { -plugin-store }
stores-link-sheet-detail = Bu bağlantıyı diğer { -device }ınızda açın ve { -plugin-store }na katılın. Yalnızca bir kez çalışır.
accounts-synced-ago = Eşitlendi: { $ago }
accounts-card-line = { $tracker } · { $status }
accounts-card-description = { $name }, { $line }. Hesaplar'ı açar
accounts-card-description-signed-out = Oturum açılmamış. Hesaplar'ı açar
accounts-signed-in-plain = Oturum açıldı
settings-summary-notify-on = Yeni bölümler açık
settings-summary-notify-off = Yeni bölümler kapalı
settings-summary-sources = { $count ->
    [one] Bir { -source }
   *[other] { $count } { -source }
 }
settings-summary-no-sources = Henüz { -source } yok
settings-summary-torrents-on = Torrent'ler açık
settings-summary-torrents-off = Torrent'ler kapalı
library-storage-on-device = Bu { -device }da { $size }
library-storage-none = Bu { -device }da hiçbir şey yok
library-detail-of = E{ $next } / { $total }
library-detail-next = E{ $next }
library-sort-last-watched = Son izlenen
library-sort-current = Geçerli
settings-notify-permission-note = { -app-name } ilk kez açtığınızda izin ister.
settings-notify-refused = Kapalı: Android, { -app-name } bildirimlerine izin vermiyor.
settings-notify-system-title = Bildirim ayarları
settings-notify-system-detail = Ses, titreşim ve kilit ekranı
settings-titles-line-signed-out = AniList oturumu kapalıyken İngilizce
settings-titles-line-account-language = { $language }, AniList hesabınızdan
settings-titles-line-follows = AniList hesabınızı izler
settings-titles-line-english = İngilizce adı olmayanlarda romaji
settings-titles-line-romaji = Japonca adlar Latin harfleriyle
settings-titles-note-account = AniList hesabınızı izler. Hesabınızın dili { $language }. AniList oturumunuz kapalıyken başlıklar İngilizce gösterilir.
settings-titles-note-follows = AniList hesabınızı izler. AniList oturumunuz kapalıyken başlıklar İngilizce gösterilir.
settings-titles-note-english-missing = { $title }, AniList'te İngilizce adı olmadığı için romaji adını korur.
settings-titles-note-english = AniList'te listelendiği gibi İngilizce adlar. Yoksa romaji.
settings-titles-note-romaji = AniList'te listelendiği gibi, Latin harflerle yazılmış Japonca adlar.
settings-index-none = Ayarlanmadı
settings-stores-with-updates = { $stores } · { $updates }
settings-seeding-best-effort = Bu { -device }da en iyi çaba: paylaşım yalnızca { -app-name } açıkken çalışır.
settings-downloads-count = { $count ->
    [one] Bir bölüm
   *[other] { $count } bölüm
 }
settings-anilist-footer = İlerleme ve liste değişiklikleri AniList'e eşitlenir. { -library-cap } AniList listelerinizi gösterir.
accounts-sign-in-again-button = Yeniden oturum aç
accounts-review-differences = { $count ->
    [one] Bir farkı incele
   *[other] { $count } farkı incele
 }
settings-licence-title = Lisans
settings-licence-value = MIT, açık kaynak ve ticari olmayan
settings-typefaces-value = Roboto Flex ve Zen Kaku Gothic New, SIL Open Font License 1.1 altında
settings-libraries-value = JNA ve AndroidX, Apache License 2.0 altında
settings-report-bug-value = GitHub'ı tarayıcınızda açar
downloads-storage-share-description = İndirmeler { $size } yer kaplıyor
settings-aside-notifications = Takip ettiğiniz bir gösterinin yeni bölümü çıktığında tek bildirim: gösterinin adı ve bölüm numarası, bölümün adı değil. Bir gösteri bunu kendi ayarlarından kapatabilir.
settings-aside-downloads = İndirmeler, bu { -device }da { -app-name }'in kendi alanında durur ve başka uygulamalar okuyamaz. Uygulamayı kaldırmak onları da siler.
settings-aside-connection-check = Bağlantı denetimi
settings-aside-sources = { -source-cap-pl } sırayla, yukarıdan aşağıya denenir. Bir gösteri, başlık sayfasından kendine özgü bir { -source } seçebilir.
settings-upscaling-description = Aynı kare. Soldaki yarısı özgün 480p, sağdaki yarısı { $mode } ile.
settings-upscaling-source = Özgün 480p
settings-upscaling-note-off-original = Özgün 480p, işlem yapılmadan sığacak şekilde ölçeklenir.
torrents-sheet-turn-on-title = Torrent'leri aç
torrents-sheet-protect-title = Torrent trafiğini koru
torrents-sheet-turn-on-button = Aç
torrents-sheet-host = Sunucu
torrents-sheet-port = Bağlantı noktası
torrents-sheet-proxy-auth-note = Kullanıcı adı ve parola yalnızca proxy istiyorsa girilir.
torrents-sheet-edit-proxy-title = Proxy'yi düzenle
title-facts-also-on = Ayrıca
title-fetch-failed-detail = AniList yanıt vermedi.
continue-resize-queue = Kuyruğun boyutunu değiştir
continue-go-to-title = Başlığa git
continue-short-left = { $episode } · { $left }
continue-airing-out = E{ $episode } · { $time } · Çıktı: { $ago }
continue-airing-in = E{ $episode } · { $time } · Kalan: { $countdown }
calendar-line-out = { $episode } · Çıktı: { $ago }
calendar-line-in = { $episode } · Kalan: { $countdown }
nav-accounts-and-settings-user = { $user }, hesaplar ve ayarlar
episode-unaired-label = Bölüm { $number }, { $title }. Henüz yayınlanmadı
episode-unaired-label-plain = Bölüm { $number }. Henüz yayınlanmadı
episode-unaired-airs-label = Bölüm { $number }, { $title }. Yayın: { $airs }. Henüz çıkmadı
episode-unaired-airs-label-plain = Bölüm { $number }. Yayın: { $airs }. Henüz çıkmadı
tv-sheet-on-network = Bu ağda
tv-sheet-not-listed = Listelenmedi
tv-sheet-scan = TV'deki kodu tara
tv-sheet-code-on = { $name } üzerindeki kod
tv-sheet-pairing-button = Eşleştiriliyor
tv-sheet-connecting = TV'ye bağlanılıyor
tv-sheet-type-code = Kodu yazın
tv-sheet-volume = TV sesi
tv-sheet-volume-down = Sesi kıs
tv-sheet-volume-up = Sesi aç
tv-sheet-volume-value = { $value } / 100
tv-sheet-nothing-playing = Hiçbir şey oynatılmıyor
tv-sheet-position = TV'deki konum
tv-sheet-position-state = { $at } / { $total }
tv-sheet-request-titled = { $title }, bölüm { $episode }
tv-sheet-request-plain = Bölüm { $episode }
tv-sheet-footer = Bir TV, bu { -device }ı { $code } koduyla listeler. Burada bir TV'yi unutursanız bu { -device } de unutulur.
tv-sheet-title-fallback = TV
episode-line-detail = E{ $number } · { $detail }
calendar-day-label = { $day }
calendar-day-label-today = { $day }, bugün
calendar-day-label-airing = { $day }, bölümler yayınlanıyor
calendar-day-label-today-airing = { $day }, bugün, bölümler yayınlanıyor
continue-show-airing-in-calendar = Bugün yayınlanan gösteriyi Takvim'de göster
continue-show-season-in-seasonal = Bu sezon yayınlanan gösteriyi Sezonluk'ta göster
search-searching-filters = { $filters } için aranıyor
episode-number-plain = Bölüm { $number }
episode-number-title-text = Bölüm { $number } · { $title }
player-position-state = { $at } / { $total }
player-sheet-finding = { -source-pl } aranıyor…
player-sheet-none = Bu bölüm için { -source } bulunamadı.
player-sheet-episodes-loading = Bölüm listesi geliyor.
player-dual-audio = { -subbed-cap } ve { -dubbed }
notif-new-episodes = { $count ->
    [one] Yeni bir bölüm
   *[other] { $count } yeni bölüm
 }
notif-hidden-text = Yeni bölüm
notif-episode-out = Bölüm { $episode } çıktı.
notif-episode-out-dubbed = Bölüm { $episode } çıktı, { -dubbed }.
notif-episode-out-next = Bölüm { $episode } çıktı. Sizin için { -up-next }: bölüm { $next }.
notif-episode-out-dubbed-next = Bölüm { $episode } çıktı, { -dubbed }. Sizin için { -up-next }: bölüm { $next }.
device-name-phone = Android telefon
device-name-tablet = Android tablet
tv-session-starting = { $tv } üzerinde başlıyor
tv-session-line = { $line } · { $tv }
widget-line-left = { $episode } · { $minutes } dk kaldı
widget-line-of = E{ $episode } / { $total }
widget-continue-title = { -continue-cap }
widget-caught-up-title = Güncelsiniz
widget-caught-up = Her şeyi izlediniz.
widget-caught-up-next = { $title } E{ $episode }, yayın: { $airs }.
widget-day-date = { $day }
widget-play-dubbed-label = Oynat, { $title }, bölüm { $episode }, { -dubbed }
tv-action-resume = Devam et: bölüm { $episode }
tv-action-resume-dubbed = Devam et: bölüm { $episode } · { -dubbed-cap }
tv-action-out = Oynat: bölüm { $episode } ({ $day } çıkacak)
tv-action-not-out = Oynat: bölüm { $episode } (henüz çıkmadı)
tv-out-today = bugün
tv-hero-resume = Devam et: { $episode }
tv-hero-start = Oynat: { $episode }
