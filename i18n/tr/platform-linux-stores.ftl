stores-device-count = { $count ->
    [one] { $count } { -device }
   *[other] { $count } { -device }
 }
stores-dialog-title = { $title }, { $store }
stores-owner-unreachable = { -plugin-store-cap }na ulaşılamadı
stores-owner-no-answer = { -plugin-store-cap } yanıt vermedi
stores-owner-no-answer-detail = Meşgul olabilir ya da bilgisayarı kapalı olabilir.
stores-owner-needs-connection = { -plugin-store-cap } bağlantı gerektiriyor
stores-owner-needs-connection-detail = Bu { -device } çevrimiçi olunca yeniden deneyin.
stores-owner-link-offline = Bağlantı oluşturmak için { -plugin-store }na bağlanmanız gerekir.
stores-send-link-until = Bu bağlantıyı gönderin. Bir kez çalışır ve { $until } kadar geçerli.
stores-open-on-detail = Ad girmeden katılırlar. Bunu kapatmak bağlantıyı durdurur; yeniden açmak yeni bir bağlantı oluşturur.
stores-open-off-detail = Yalnızca davet ettiğiniz kişiler katılabilir. Açıldığında bağlantısı olan herkes, ad girmeden katılır.
stores-open-no-names = Hiçbir ad tutulmaz.
stores-open-keep-working = Siz kaldırana kadar çalışmaya devam ederler.
stores-device-seen = Son görülme: { $when }
stores-device-never-seen = Katıldığından beri görülmedi
stores-publish-short = Bir { -plugin } yayımla
stores-publish-none-hint = Oluşturduğunuz ya da kurduğunuz bir { -plugin } burada görünür.
stores-not-published = { -plugin-store-cap }nda henüz yok
stores-store-offers = { -plugin-store-cap }nda { $version } var.
stores-published-current = { -plugin-store-cap }nda güncel
stores-published-state = Yayımlandı
stores-publish-label = { $name } { $version } yayımla
