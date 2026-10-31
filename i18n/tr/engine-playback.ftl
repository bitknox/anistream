notice-reason = { $reason }
cause-no-sources = Hiçbir { -source } ayarlanmadı. { -source-cap-pl } ekranını açıp bir { -source } ekleyin.
notice-not-aired = Bölüm { $episode }, { $when } yayınlanacak.
notice-not-aired-yet = Bölüm { $episode } henüz yayınlanmadı. Bölüm { $next }, { $when } yayınlanacak.
cause-airs-today = bugün saat { $time }
cause-airs-tomorrow = yarın saat { $time }
cause-airs-in-days = { $days ->
    [one] 1 gün içinde saat { $time }
   *[other] { $days } gün içinde saat { $time }
 }
cause-airs-on-date = { $date } tarihinde saat { $time }
cause-no-playable-stream = { -source } yanıt verdi ama oynatılacak bir şey göndermedi. Başka birini deneyin.
cause-title-no-match = Bu başlık eşleştirilemedi: { $detail }
cause-source-gone = { -source } { $source } artık kullanılamıyor. Başka birini seçin.
cause-release-failed = Bu { -release } başlatılamadı: { $detail }
cause-sources-timed-out = { -source-cap-pl } yanıt vermek için çok uzun bekledi. Yeniden deneyin ya da başka bir { -source } seçin.
cause-no-source-has-episode = Bölüm { $episode } hiçbir { -source } tarafından bulunamadı: { $detail }
cause-source-cannot-download = Bu { -source } bölümleri akış olarak sunuyor ama indirilemiyor.
cause-no-source-could-list-show = Hiçbir { -source } bu diziyi listeleyemedi, bu yüzden indirilemiyor.
cause-subtitles-merged = { $tracks ->
    [one] Bir altyazı izi videoya eklendi.
   *[other] { $tracks } altyazı izi videoya eklendi.
 }
cause-subtitles-left-apart = Altyazılar ayrı dosyalar olarak kaldı. { $reason }
cause-already-in-downloads = { $count ->
    [one] Bir bölüm zaten indirmelerde.
   *[other] { $count } bölüm zaten indirmelerde.
 }
status-looking-up-title = Başlık aranıyor…
status-loading-sources = Yükleniyor: { -source-pl }…
status-searching-sources = Bölüm { $episode } için { -source-pl } aranıyor…
notice-player-failed-to-start = mpv başlatılamadı. Tek başına çalışıp çalışmadığını kontrol edin. ({ $reason })
notice-stream-no-video = Bu akıştan görüntü gelmedi. Bir sonraki akış deneniyor ({ $next }/{ $total }).
notice-player-could-not-play = mpv bunu oynatamadı. { $reason }
notice-player-exited-silently = mpv bir şey oynatmadan ve neden belirtmeden kapandı. Başka bir { -source } deneyin.
notice-player-not-playing = mpv { $seconds } sn geçmesine rağmen oynatmaya başlamadı. Bu { -source } veri içermiyor olabilir. mpv'nin bir dosyayı tek başına oynatıp oynatmadığını kontrol edin.
notice-no-player = Bu { -app-name } sürümü video oynatamıyor.
notice-mpv-missing = { $binary } bulunamadı. Bölümleri oynatmak için mpv'yi kurun.
notice-mpv-old = mpv { $version } oynatabiliyor, ama { -app-name } oynatıcısının atlama düğmesi, { -up-next-cap } kartı, menüleri ve küçük resimleri için mpv 0.36 ya da daha yenisi gerekli.
notice-subtitles-unavailable = Bu { -source } altyazıları yüklenemedi. Yeniden denemek için başka bir { -source } seçin.
notice-opened-in-browser = Tarayıcınızda açıldı.
notice-could-not-open = Bu bağlantı açılamadı. { $reason }
notice-trailer-playing = Fragman mpv'de oynatılıyor.
notice-trailer-opened = Fragman tarayıcınızda açıldı.
notice-trailer-failed = Fragman açılamadı. { $reason }
notice-filler-skipped = Dolgu bölüm atlandı. { -up-next-cap } bölüm { $episode }.
notice-audio-now-subbed = Ses şimdi { -subbed }.
notice-audio-now-dubbed = Ses şimdi { -dubbed }.
notice-already-first-episode = Zaten ilk bölümde.
notice-last-episode = Sonraki bölüm yok. Bölüm { $episode } son bölüm.
notice-no-next-episode = Sonraki bölüm yok.
notice-no-previous-episode = Önceki bölüm yok.
notice-match-reset-failed = Eşleşme sıfırlanamadı. { $reason }
notice-match-save-failed = Bu eşleşme kaydedilemedi. { $reason }
notice-match-did-not-stick = Bu eşleşme tutmadı. Bu { -source } değişmiş olabilir. Eşleşmeyi yeniden seçmeyi deneyin.
notice-source-search-empty = “{ $query }” için sonuç bulunamadı. Farklı bir arama deneyin.
match-ruled-out-format = Farklı biçim
match-ruled-out-episodes = Bu başlıktan daha fazla bölüm var
match-ruled-out-year = Farklı yıl
match-search-timed-out = Yanıt vermesi çok uzun sürdü.
match-search-source-off = Açık değil.
notice-source-pinned = { $source } bu başlık için kullanılacak.
notice-source-automatic = Otomatik { -source } seçimine dönüldü.
notice-source-choice-failed = Bu seçim kaydedilemedi. { $reason }
notice-no-source-could-list = Hiçbir { -source } bu diziyi listeleyemedi: { $reason }
notice-episodes-failed = Bölümler yüklenemedi. { $reason }
notice-match-uncertain = { $source } bu olabilecek birden fazla dizi buldu. Biri seçilene kadar bölümler şimdilik AniList'ten gösteriliyor.
notice-continue-hide-failed = Bu başlık { -continue-cap } listesinden kaldırılamadı. { $reason }
notice-download-queued = { $title }, bölüm { $episode } kuyruğa eklendi.
notice-download-already-queued = { $title }, bölüm { $episode } zaten indirmelerde.
notice-download-starting-now = { $title }, bölüm { $episode } şimdi başlatılıyor.
notice-download-cancelled = İndirme iptal edildi.
notice-download-file-deleted = İndirme ve dosyası silindi.
notice-download-removed-file-gone = İndirme kaldırıldı. Dosyası zaten yoktu.
notice-download-removed-file-kept = İndirme kaldırıldı, ama dosyası silinemedi. { $reason }
notice-download-removed = İndirme kaldırıldı.
notice-downloads-cleared = { $count ->
    [one] Biten bir indirme temizlendi.
   *[other] Biten { $count } indirme temizlendi.
 }
notice-download-finished = { $title }, bölüm { $episode } indirildi.
notice-download-finished-note = { $title }, bölüm { $episode } indirildi. { $note }
notice-download-failed = { $title }, bölüm { $episode } indirilemedi. { $reason }
notice-downloads-none-queued = Kuyruğa hiçbir şey eklenmedi.
notice-downloads-none-queued-stopped = Kuyruğa hiçbir şey eklenmedi. { $reason }
notice-downloads-none-new = Kuyruğa yeni bir şey eklenmedi. { $existing }
notice-downloads-none-new-stopped = Kuyruğa yeni bir şey eklenmedi. { $existing } { $reason }
notice-episodes-queued-existing = { $count ->
    [one] Bir bölüm kuyruğa eklendi. { $existing }
   *[other] { $count } bölüm kuyruğa eklendi. { $existing }
 }
notice-episodes-queued-stopped = { $count ->
    [one] Bir bölüm kuyruğa eklendi, sonra durdu. { $reason }
   *[other] { $count } bölüm kuyruğa eklendi, sonra durdu. { $reason }
 }
notice-episodes-queued-existing-stopped = { $count ->
    [one] Bir bölüm kuyruğa eklendi, sonra durdu. { $existing } { $reason }
   *[other] { $count } bölüm kuyruğa eklendi, sonra durdu. { $existing } { $reason }
 }
notice-keep-next-full = Sonraki bölümleri tutmak için yeterli boş alan yok. Daha fazla indirmek için biraz alan açın.
party-needs-server = Önce { -watch-party } için bir sunucu seçin, syncplay.server olarak. { $server } sunucusu { $operator } tarafından işletiliyor.
party-choose-server = Önce { -watch-party } için bir sunucu seçin. { $server } sunucusu { $operator } tarafından işletiliyor.
notice-no-party = { -watch-party-cap-pl } bu { -app-name } sürümünde yok.
party-needs-room = { -watch-party-cap } için bir oda gerekli. Önce birini seçin.
party-server-lost = { -watch-party-cap } sunucusuyla bağlantı koptu. Yeniden deneniyor.
party-rejoined = { -watch-party-cap }ye yeniden bağlandı.
party-left = { -watch-party-cap }den ayrıldı.
