stores-needs-larger-terminal = Daha büyük bir terminal gerekiyor
stores-list-heading = { -plugin-store-cap-pl }
stores-page-heading = { -plugin-store-cap }
stores-members-placeholder-heading = Üyeler
stores-column-store = { -store-cap }
stores-column-plugins = { -plugin-cap-pl }
stores-column-you = Siz
stores-column-state = Durum
stores-none-joined = Henüz katılınmadı
stores-role-owner = Sahip
stores-role-member = Üye
stores-join-row = + Bir { -store }ya katıl
stores-state-revoked = Erişim kaldırıldı
stores-state-offline = Çevrimdışı
stores-state-up-to-date = Güncel
stores-state-updates = { $count ->
    [one] { $count } güncelleme
   *[other] { $count } güncelleme
 }
stores-footer-revoked = Bu { -store } artık bu { -device }ı kabul etmiyor. Sahibi yeni bir davet gönderebilir.
stores-footer-offline = { $url } ({ $reason })
stores-key = Anahtar { $fingerprint }
stores-joined-without-name = Ad vermeden katıldı
stores-joined-as = Şu adla katıldı: { $name }
stores-footer-join = Size verilen daveti ya da { -store } bağlantısını yapıştırın
stores-owner-tag = Sahip
stores-anyone-can-join = Bağlantısı olan herkes katılabilir
stores-note-offline = Çevrimdışı; son gönderdiği liste gösteriliyor ({ $reason })
stores-note-revoked = Bu { -store } artık bu { -device }ı kabul etmiyor. Daha önce yüklenenler çalışmaya devam eder.
stores-column-plugin = { -plugin-cap }
stores-column-version = { -release-cap }
stores-empty-owner = Henüz bir şey yayımlanmadı. Aşağıdan bir { -plugin } yayımlayın.
stores-empty-member = Henüz bir şey yayımlanmadı
stores-line-link-device = Başka bir { -device } bağla
stores-line-invite = Birini davet et
stores-line-members = Üyeler
stores-line-who-can-join = Kimler katılabilir
stores-line-publish = Bir { -plugin } yayımla
stores-line-admin = Yönetici sayfasını aç
stores-line-leave = Bu { -store }dan ayrıl
stores-plugin-not-installed = Yüklenmedi
stores-plugin-installed = Yüklendi
stores-plugin-update-from = { $from } sürümünden güncelleme
stores-plugin-added-by-hand = Elle eklendi
stores-plugin-from-another-store = Başka bir { -store }dan
stores-footer-update-hosts = Güncelleme ayrıca { $hosts } ile iletişim kurmak istiyor
stores-footer-may-contact = Şunlarla iletişim kurabilir: { $hosts }
stores-footer-link-device = Başka bir bilgisayarınıza sizin olarak katılan tek kullanımlık bağlantı
stores-footer-invite = Yeni biri için tek kullanımlık bağlantı
stores-footer-members = Kimlerin katıldığı ve birinin erişimini kaldırma
stores-footer-close-join = ↵ Yalnızca davet ettiğiniz kişilerin katılmasına izin ver ve bağlantıyı durdur
stores-footer-open-join = ↵ { -store }nın bağlantısı olan herkesin adını vermeden katılmasına izin ver
stores-footer-publish = Bu bilgisayarın { -plugin } klasöründen bir { -plugin } sun
stores-footer-admin = Üyeleri, davetleri ve { -plugin-pl } tarayıcınızda yönetin
stores-footer-leave = { -plugin-cap-pl }, bu { -store }dan yüklenenler, bu bilgisayarda kalır ve güncellenmeyi durdurur.
stores-who-open = Bağlantısı olan herkes
stores-who-invite-only = Yalnızca davet ettiğiniz kişiler
stores-label-offers = Sunulan
stores-offers-count = { $count ->
    [one] { $count } { -plugin }
   *[other] { $count } { -plugin }
 }
stores-offered-plugin = { $name } { $version }  şunlarla iletişim kurabilir: { $hosts }
stores-offered-installed = Yüklü
stores-offered-installed-version = { $from } sürümü yüklü
stores-offered-added-by-hand = Elle eklenmiş
stores-offered-from-other = { $other } üzerinden
stores-offered-more = ve { $count } tane daha
stores-hosts-none = hiçbiri
stores-loading = Yükleniyor…
stores-member-you = { $name } (siz)
stores-column-name = Ad
stores-column-role = Rol
stores-column-devices = { -device-cap-pl }
stores-column-last-seen = Son görülme
stores-members-heading = Üyeler · { $name }
stores-unnamed = { -plugin-store-cap }
stores-invites-heading = Henüz kullanılmamış davetler
stores-invite-another-device = { $name }, başka bir { -device }
stores-seen-never = Hiçbir zaman
stores-ago-just-now = Az önce
stores-ago-today = Bugün
stores-ago-yesterday = Dün
stores-ago-days = { $days ->
    [one] { $days } gün önce
   *[other] { $days } gün önce
 }
stores-ago-inline-just-now = az önce
stores-ago-inline-today = bugün
stores-ago-inline-yesterday = dün
stores-ago-inline-days = { $days ->
    [one] { $days } gün önce
   *[other] { $days } gün önce
 }
stores-expired = Süresi doldu
stores-expires-today = Bugün sona eriyor
stores-expires-tomorrow = Yarın sona eriyor
stores-expires-days = { $days ->
    [one] { $days } gün içinde sona erer
   *[other] { $days } gün içinde sona erer
 }
stores-open-heading = Bağlantısı olan herkes katılabilir · Bu bağlantıyla katılan: { $joined }
stores-closed-heading = Kapatılmadan önce bu bağlantıyla katılanlar · { $joined }
stores-joined-devices = { $count ->
    [one] { $count } { -device }
   *[other] { $count } { -device }
 }
stores-open-link-row = { -store }nın bağlantısı
stores-open-link-note = Göster ve kopyala
stores-anonymous-joined = { $when } katıldı
stores-anonymous-seen = { $when } görüldü
stores-anonymous-never-seen = Henüz görülmedi
stores-remove-devices = { $count ->
    [one] Kaldır
   *[other] Tümünü kaldır ({ $count })
 }
stores-footer-you = Siz sahipsiniz. Bir { -store } her zaman sahibini korur.
stores-footer-remove-member = ↵ Erişimini kaldır
stores-footer-cancel-invite = ↵ Bu daveti iptal et
stores-footer-show-link = ↵ Bağlantıyı göster, kopyalamak ve paylaşmak için
stores-footer-remove-anonymous = ↵ Bu { -device }ı kaldır; yalnızca katıldığı zamana göre tanınır
stores-footer-remove-all-anonymous = ↵ Bağlantıyla katılan bütün { -device-pl }ı kaldır
stores-link-copied = Terminal izin verdiğinde panoya kopyalandı. Kopyalamak için seçin.
stores-link-select = Kopyalamak için seçin.
stores-open-link-warning = Bağlantısı olan herkes adını vermeden, istediği kadar katılabilir.
stores-open-link-lasts = { -store } yalnızca davetle katılıma dönene kadar çalışır.
stores-invite-works-once = Yalnızca bir kez çalışır. { $expiry }.
stores-invite-preview-safe = Tarayıcıda açmak katılmanın nasıl yapılacağını gösterir ve daveti kullanmaz.
stores-confirm-join = Katılmayı onayla: { $name }
stores-confirm-leave = Ayrılmayı onayla: { $name }
stores-confirm-remove-member = Üyeyi kaldırmayı onayla: { $name }
stores-confirm-cancel-invite = Daveti iptal etmeyi onayla: { $name }
stores-confirm-unpublish = Sunmayı durdurmayı onayla: { $name }
stores-confirm-open = Bağlantıyla herkesin katılmasını onayla: { $name }
stores-confirm-close = Yalnızca davetlilerin katılmasını onayla: { $name }
stores-confirm-remove-device = Bu { -device }ı kaldırmayı onayla
stores-confirm-remove-devices = { $count ->
    [one] Bağlantıyla katılan { -device }ı kaldırmayı onayla
   *[other] Bağlantıyla katılan { $count } { -device }ı kaldırmayı onayla
 }
stores-confirm-fallback = Onayla
stores-label-address = Adres
stores-label-key = Anahtar
stores-label-run-by = Yönetici
stores-label-device = { -device-cap }
stores-key-matches-link = { $fingerprint }  (bağlantıyla eşleşiyor)
stores-key-matches-invite = { $fingerprint }  (davetle eşleşiyor)
stores-join-open = Bağlantısı olan herkes katılabilir.
stores-join-anonymous = Adınızı vermeden katılırsınız ve aynı bağlantıyla başka bir { -device } katılır.
stores-join-already = Zaten katıldınız. Yeniden katılmak, bu { -device }a yeni bir belirteç verir.
stores-join-device-note = { -store }nın sahibi bu adı görür. Değiştirmek için yazın.
stores-leave-plugins = { -plugin-cap-pl }, bu bilgisayarda kalır ve güncellenmeyi durdurur.
stores-leave-revoked = Bu { -device }ın { -store }ya erişimi kaldırıldı.
stores-remove-member-devices = { $count ->
    [one] Onun { -device }ı bu { -store }yla hemen çalışmaz.
   *[other] Onların { $count } { -device }ı bu { -store }yla hemen çalışmaz.
 }
stores-remove-member-plugins = { -plugin-cap-pl }, zaten yükledikleri, kendi bilgisayarlarında kalır.
stores-cancel-invite-link = Bağlantı çalışmayı durdurur.
stores-unpublish-members = Onu yükleyen üyeler kullanmaya devam eder, ama güncellenmez.
stores-open-anyone = Bağlantısı olan herkes { -store }ya adını vermeden katılabilir.
stores-open-link-new = Bağlantı her seferinde yenilenir, bu yüzden önceki bağlantı çalışmaz.
stores-open-members-see = Üyeler { -store }nın açık olduğunu görür.
stores-close-link = Bağlantı çalışmayı durdurur ve davet olmadan kimse yeni katılamaz.
stores-close-devices = { -device-cap-pl }, bu bağlantıyla katılanlar, üyeler bölümünden kaldırılana kadar çalışmaya devam eder.
stores-anonymous-one-stops = Bu { -store }yla çalışması hemen durur.
stores-anonymous-all-stop = Bu { -store }yla çalışmaları hemen durur.
stores-anonymous-plugins = { -plugin-cap-pl } zaten yüklü oldukları yerde kalır.
stores-anonymous-still-joins = { -store } açık olduğu sürece bağlantı herkesin katılmasına izin verir.
stores-plugin-may-contact = Şunlarla iletişim kurabilir: { $hosts }
stores-plugin-cannot = Dosya okuyamaz, soket açamaz ya da ortamınızı göremez.
stores-plugin-update-adds = Güncelleme, iletişim kurabileceği yerlere { $hosts } ekler.
stores-plugin-added-by-hand-note = { $id }.wasm zaten { -plugin } klasöründe, elle eklenmiş.
stores-plugin-other-store-note = Başka bir { -store } üzerinden yüklendi, o da güncel tutar.
stores-choice-install = Yükle
stores-choice-update = { $version } sürümüne güncelle
stores-choice-replace = Elle eklenen kopyayı değiştir
stores-choice-remove = Bu bilgisayardan kaldır
stores-choice-unpublish = Bu { -store }da sunmayı durdur
stores-choice-cancel = İptal
stores-publish-new = Yeni
stores-publish-published = Yayımlandı
stores-publish-replaces = { $version } sürümünün yerine geçer
stores-toast-invite-checked = Davet geçerli, ama başka bir işlem açık. Katılmak için yeniden yapıştırın.
stores-toast-nothing-to-publish = { -plugin } klasöründe yayımlanacak { -plugin-pl } yok. Nerede olduğunu görmek için "{ $command }" komutunu çalıştırın.
stores-toast-that-is-you = Bu sizsiniz. Bir { -store } her zaman sahibini korur.
