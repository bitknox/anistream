queue-episodes-left = { $count ->
    [one] { $title }: bir bölüm kaldı
   *[other] { $title }: { $count } bölüm kaldı
 }
episodes-count = { $count ->
    [one] { $count } bölüm
   *[other] { $count } bölüm
 }
progress-episodes = { $total ->
    [one] { $watched } / { $total } bölüm
   *[other] { $watched } / { $total } bölüm
 }
duration-days-hours = { $days } g { $hours } sa
group-in-progress = Devam eden
group-new-this-week = Bu hafta yeni
continue-empty-title = Henüz izlenen yok
continue-empty-detail = Sezonluk'tan bir şey seçin ya da bir başlık arayın.
continue-empty-action = Sezonluk'a göz at
continue-load-failed = { -continue-cap } yüklenemedi
continue-heading = { -continue-cap }
continue-airing-today = Bugün yayınlananlar
continue-this-season = Bu sezon
continue-hide = { -continue-cap } listesinden gizle
continue-hidden = { -continue-cap } listesinden gizlendi.
continue-drop = Durumu Bırakıldı yap
continue-more-for = { $title } için diğer seçenekler
continue-row-resume = Devam et: E{ $episode } · { $clock }
continue-row-resume-dubbed = Devam et: E{ $episode } { -dubbed } · { $clock }
continue-row-start = Oynat: E{ $episode }
continue-row-start-dubbed = Oynat: E{ $episode } { -dubbed }
continue-row-start-aired = Oynat: E{ $episode } · yayınlandı: { $ago }
continue-row-caught-up = Yeni bölüm yok · E{ $episode }: { $when }
continue-row-caught-up-plain = Yeni bölüm yok
continue-widget-connect = Burada görmek için AniList'i bağlayın.
continue-caught-up = Yeni izlenecek bir şey kalmadı.
title-button-resume = Bölüm { $episode } devam et · { $time }
title-button-play = Bölüm { $episode } oynat
title-button-add-source = Oynatmak için bir { -source } ekleyin
library-status-watching = İzleniyor
library-status-planning = Planlandı
library-status-completed = Tamamlandı
library-status-paused = Ara verildi
library-status-dropped = Bırakıldı
library-sort-updated = Son güncelleme
library-sort-title = Başlık
library-sort-score = Puan
library-sort-button = Sıralama: { $sort }
library-sort-label = { $sort } ile sırala
library-signed-out-title = Listeleriniz AniList'ten gelir
library-signed-out-detail = İzlediklerinizi ve kaldığınız yeri eşitlemek için giriş yapın.
library-signed-out-action = AniList'e giriş yap
library-list-empty = { $list } listeniz boş
library-list-empty-detail = AniList'te bu listeye eklediğiniz başlıklar burada görünür.
library-list-load-failed = { $list } listeniz yüklenemedi
library-loading = Listeleriniz yükleniyor
library-add-to-list = Listeye ekle
library-add-planning = Planlamaya ekle
library-added-planning = Planlamaya eklendi.
library-on-your-list = Listenizde
library-in-your-list = Listenizde
progress-episodes-short = E{ $done } / { $total }
progress-episode-short = E{ $done }
continue-row-start-aired-dubbed = Oynat: E{ $episode } { -dubbed } · yayınlandı: { $ago }
up-next-heading = Sizin için { -up-next-cap }
up-next-starts = Başlıyor: { $date }
up-next-reason-new-season = Yeni sezon
up-next-reason-planning = Planlama listenizde
up-next-reason-recommended = Bitirdiğiniz için: { $title }
up-next-reason-recommended-plain = Sizin için önerilen
up-next-because-detail = AniList'te { $title } beğenenler tarafından önerildi
up-next-reason-airing-today = Bugün yayınlanıyor
up-next-add-planning = Planlamaya ekle
up-next-not-interested = İlgilenmiyorum
up-next-row-starts = { $reason } · { $starts }
up-next-hidden = { $title }, { -up-next-cap } listesinden gizlendi.
up-next-hidden-until-airs = { $title } yayına başlayana kadar gizlendi.
up-next-broadcast = Bölüm { $episode } · { $when }
up-next-out = yayınlandı: { $ago }
