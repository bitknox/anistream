notice-signed-in = Oturum açıldı: { $tracker }.
notice-signed-in-keychain = Oturum açıldı: { $tracker }. Erişim jetonu işletim sistemi anahtarlığında saklanıyor.
notice-signed-in-file = Oturum açıldı: { $tracker }. Anahtarlık yok. Erişim jetonu bu { -device } üzerinde, yalnızca bu kullanıcının okuyabildiği özel bir dosyada.
notice-signed-in-unsaved = Oturum açıldı: { $tracker }, ama jeton kaydedilemedi. Bir dahaki sefer yeniden oturum açmanız gerekecek. { $reason }
notice-token-not-saved = Jeton kaydedilemedi. Yeniden oturum açmayı deneyin. { $reason }
notice-sign-in-failed = Oturum açılamadı: { $tracker }. Yeniden deneyin. { $reason }
notice-sign-in-stopped = Oturum açma tamamlanmadan durdu: { $tracker }. Yeniden deneyin. { $reason }
notice-token-exchange-failed = Jeton alınamadı: { $tracker }. Yeniden oturum açmayı deneyin. { $reason }
notice-browser-not-opened = Oturum açma için tarayıcı açılamadı. Bir tarayıcının kurulu olup olmadığını denetleyin ve yeniden deneyin. { $reason }
notice-sign-in-not-waiting = Bunu bekleyen bir oturum açma işlemi yok ({ $tracker }). Oturum açmayı yeniden başlatın.
notice-sign-in-wrong-request = Bu oturum açma ({ $tracker }) bu istekten dönmedi. Oturum açmayı yeniden başlatın.
notice-sign-in-no-token = Oturum açma ({ $tracker }) jetonsuz döndü. Yeniden deneyin.
notice-mal-client-missing-build = MyAnimeList oturumu tamamlanamadı: bu derlemede bunun için bir istemci yok.
notice-kitsu-signs-in-on-accounts = Kitsu oturumu Hesaplar ekranından açılır.
notice-sign-in-unsupported = { $tracker } için oturum açma henüz desteklenmiyor.
notice-sign-in-needs-redirect = { $tracker } için oturum açmak, bu uygulamanın alamayacağı bir tarayıcı yönlendirmesi gerektirir.
notice-anilist-implicit-flow = Bu AniList oturum açma yöntemi yalnızca { -app-name } uygulamasında çalışır. Burada oturum açmak için yapılandırma dosyasında { $setting } değerini "loopback" olarak ayarlayın.
notice-anilist-client-missing = AniList için istemci kimliği ve gizli anahtar gerekiyor. Yapılandırma dosyasında { $client } ve { $secret } değerlerini ayarlayın, sonra yeniden oturum açın. Oturum açma adresini almak için { -app-name } --login-url komutunu çalıştırın.
notice-anilist-paste-flow = Burada kod yapıştırmak mümkün değil. Çıkıp bunun yerine { -app-name } --login komutunu çalıştırın.
notice-mal-client-missing = MyAnimeList için istemci kimliği gerekiyor. myanimelist.net/apiconfig adresinde "other" türünde bir uygulama kaydedin, sonra yapılandırma dosyasında { $setting } değerini ayarlayın.
status-waiting-for-redirect = { $tracker } yönlendirmesi bekleniyor…
notice-mal-no-refresh-token = MyAnimeList yenileme jetonu vermedi. Yaklaşık bir ay içinde yeniden oturum açmanız gerekecek.
notice-client-id-missing = Önce yapılandırma dosyasında { $setting } değerini ayarlayın.
notice-signed-out = Oturum kapatıldı: { $tracker }.
notice-signed-out-queue = { $count ->
    [one] Oturum kapatıldı: { $tracker }. Bekleyen bir değişiklik, bir sonraki oturum açmada gönderilmek üzere saklandı.
   *[other] Oturum kapatıldı: { $tracker }. Bekleyen { $count } değişiklik, bir sonraki oturum açmada gönderilmek üzere saklandı.
 }
notice-sign-out-failed = Oturum kapatılamadı: { $tracker }. Yeniden deneyin. { $reason }
notice-not-saved-because = Bu ayar kaydedilemedi. { $cause }
notice-protection-saved = Torrent koruması kaydedildi.
notice-protection-not-checked = Torrent koruması denetlenemedi. { $cause }
notice-proxy-password-not-kept = Vekil sunucu parolası saklanamadı. Kaydetmeyi denemek için yeniden girin. { $reason }
notice-show-setting-refused = Bu ayar kaydedilemedi. { $value }, { $setting } için geçerli bir değer değil.
notice-show-not-reset = Bu dizinin ayarları sıfırlanamadı. { $reason }
notice-show-no-such-subtitles = Bu altyazı seçimi kaydedilemedi. Bu dizi bu altyazıları daha önce kullanmadı.
notice-torrents-waiting-for-check = Torrentler bağlantı denetiminin yeniden geçmesini bekliyor.
notice-mapping-data-refreshed = Eşleme verileri yenilendi.
notice-mapping-data-failed = Eşleme verileri yenilenemedi ({ $datasets }). { $wait } içinde yeniden denenecek. { $cause }
notice-episode-aired = { $title }, { $episode }. bölüm, siz yokken yayınlandı.
notice-episodes-aired = { $count ->
    [one] İzlediğiniz dizilerden bir yeni bölüm yayınlandı. Takvimde görebilirsiniz.
   *[other] İzlediğiniz dizilerden { $count } yeni bölüm yayınlandı. Takvimde görebilirsiniz.
 }
notice-update-available = { $tag } çıktı. Kurmak için { -app-name } --update komutunu çalıştırın.
cause-confirm-no-protection = Önce korumasız kalmak istediğinizi onaylayın.
cause-confirm-warn-only = Önce yalnızca uyarılmak istediğinizi onaylayın.
cause-proxy-needs-address = Vekil sunucu için bir ana makine ve bir bağlantı noktası gerekir.
cause-proxy-address-unusable = Vekil sunucunun adresi denetim için kullanılamıyor.
cause-server-unreachable = Sunucuya ulaşılamadı.
