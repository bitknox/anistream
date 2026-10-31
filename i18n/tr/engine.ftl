notice-saved = Kaydedildi: { $label }.
notice-not-saved = Bu ayar kaydedilemedi. { $reason }
notice-title-load-failed = Bu başlık yüklenemedi. Bağlantınızı denetleyin ve yeniden deneyin.
notice-playback-failed = Oynatma başarısız oldu. Başka bir { -source } deneyin.
notice-engine-fault = { -app-name } içinde bir sorun oluştu. Uygulama çalışmaya devam etti, ama bir ekran donmuş görünüyorsa çıkıp uygulamayı yeniden açın.
notice-database-set-aside = Veritabanı bozulduğu için { -app-name } yenisini başlattı. Eski dosya { $path } konumunda saklanıyor. { -tracker-pl }nde tutulanlar bir sonraki eşitlemede geri gelir.
start-failed-folder = { -app-name } { $path } klasörünü oluşturamadı. Diskte yer olduğunu ve klasöre yazılabildiğini denetleyin, sonra yeniden deneyin. { $reason }
start-failed-config = Yapılandırma dosyası ({ $path }) { -app-name } için kullanılamayan bir ayar içeriyor: { $problem }. Ayarı düzeltin ya da kaldırın, sonra yeniden deneyin.
config-reset-failed = Ayarları sıfırlamak için yapılandırma dosyası ({ $path }) kenara taşınamadı. { $reason }
start-failed-database = { -app-name } veritabanı dosyasını açamadı ({ $path }). Bu klasörde başka bir { -app-name } uygulaması açıksa onu kapatıp yeniden deneyin. { $reason }
start-failed-network = { -app-name }, config.toml dosyasındaki ağ ayarlarıyla ağ bağlantısını kuramadı. { $reason }
start-failed = { -app-name } başlatılamadı. Çıkın ve yeniden açın. { $reason }
start-failed-remote = Uzaktan kumanda başlatılamadı. { $reason }
start-failed-player = Oynatıcı başlatılamadı. { $reason }
notice-episodes-queued = { $count ->
    [one] Bir bölüm indirme kuyruğuna eklendi.
   *[other] { $count } bölüm indirme kuyruğuna eklendi.
 }
