stores-needs-larger-terminal = ターミナルが小さすぎます
stores-list-heading = { -plugin-store-cap-pl }
stores-page-heading = { -plugin-store-cap }
stores-members-placeholder-heading = メンバー
stores-column-store = { -store-cap }
stores-column-plugins = { -plugin-cap-pl }
stores-column-you = あなた
stores-column-state = 状態
stores-none-joined = まだ参加していません
stores-role-owner = オーナー
stores-role-member = メンバー
stores-join-row = + { -store }に参加
stores-state-revoked = アクセス削除済み
stores-state-offline = オフライン
stores-state-up-to-date = 最新の状態
stores-state-updates = { $count ->
   *[other] { $count }件の更新
 }
stores-footer-revoked = この{ -store }は、この{ -device }を受け付けていません。オーナーが新しい招待を送れます。
stores-footer-offline = { $url }（{ $reason }）
stores-key = キー{ $fingerprint }
stores-joined-without-name = 名前なしで参加
stores-joined-as = { $name }として参加
stores-footer-join = 受け取った招待か{ -store }のリンクを貼り付けてください
stores-owner-tag = オーナー
stores-anyone-can-join = リンクを知っている人なら誰でも参加できます
stores-note-offline = オフラインです。最後に受け取った一覧を表示しています（{ $reason }）
stores-note-revoked = この{ -store }は、この{ -device }を受け付けていません。インストール済みのものは引き続き動きます。
stores-column-plugin = { -plugin-cap }
stores-column-version = バージョン
stores-empty-owner = まだ何も公開されていません。下から{ -plugin }を公開できます。
stores-empty-member = まだ何も公開されていません
stores-line-link-device = 別の{ -device }をリンクで追加
stores-line-invite = 招待を送る
stores-line-members = メンバー
stores-line-who-can-join = 参加できる人
stores-line-publish = { -plugin }を公開
stores-line-admin = 管理ページを開く
stores-line-leave = この{ -store }から退出
stores-plugin-not-installed = 未インストール
stores-plugin-installed = インストール済み
stores-plugin-update-from = { $from }から更新
stores-plugin-added-by-hand = 手動で追加
stores-plugin-from-another-store = 別の{ -store }から
stores-footer-update-hosts = この更新は{ $hosts }への接続も求めています
stores-footer-may-contact = 接続できるホスト：{ $hosts }
stores-footer-link-device = 自分として参加する、別のパソコン用の一度限りのリンク
stores-footer-invite = 新しい人のための一度限りのリンク
stores-footer-members = 参加者の一覧と、アクセス権の削除
stores-footer-close-join = ↵ 招待された人だけが参加できるようにし、リンクを止めます
stores-footer-open-join = ↵ { -store }のリンクを知っている人が、名前なしで参加できるようにします
stores-footer-publish = この端末の{ -plugin }フォルダーから{ -plugin }を公開
stores-footer-admin = ブラウザーでメンバー、招待、{ -plugin-pl }を管理
stores-footer-leave = この{ -store }からインストールした{ -plugin-cap-pl }は、この端末に残り、更新されなくなります
stores-who-open = リンクを知っている人なら誰でも
stores-who-invite-only = 招待された人だけ
stores-label-offers = 提供
stores-offers-count = { $count ->
   *[other] { $count }件の{ -plugin-pl }
 }
stores-offered-plugin = { $name } { $version }  接続先：{ $hosts }
stores-offered-installed = インストール済み
stores-offered-installed-version = { $from }がインストール済み
stores-offered-added-by-hand = 手動で追加
stores-offered-from-other = { $other }から
stores-offered-more = ほか{ $count }件
stores-hosts-none = なし
stores-loading = 読み込み中…
stores-member-you = { $name }（あなた）
stores-column-name = 名前
stores-column-role = 役割
stores-column-devices = { -device-cap-pl }
stores-column-last-seen = 最終確認
stores-members-heading = メンバー · { $name }
stores-unnamed = { -plugin-store-cap }
stores-invites-heading = 未使用の招待
stores-invite-another-device = { $name }、別の{ -device }
stores-seen-never = 未確認
stores-ago-just-now = たった今
stores-ago-today = 今日
stores-ago-yesterday = 昨日
stores-ago-days = { $days ->
   *[other] { $days }日前
 }
stores-ago-inline-just-now = たった今
stores-ago-inline-today = 今日
stores-ago-inline-yesterday = 昨日
stores-ago-inline-days = { $days ->
   *[other] { $days }日前
 }
stores-expired = 期限切れ
stores-expires-today = 今日期限切れ
stores-expires-tomorrow = 明日期限切れ
stores-expires-days = { $days ->
   *[other] { $days }日後に期限切れ
 }
stores-open-heading = リンクを知っている人なら誰でも参加できます · { $joined }が参加済み
stores-closed-heading = リンクを閉じる前に参加 · { $joined }
stores-joined-devices = { $count ->
   *[other] { $count }台の{ -device-pl }
 }
stores-open-link-row = { -store }のリンク
stores-open-link-note = 表示してコピー
stores-anonymous-joined = 参加：{ $when }
stores-anonymous-seen = 確認：{ $when }
stores-anonymous-never-seen = まだ確認されていません
stores-remove-devices = { $count ->
   *[other] すべて（{ $count }台）を削除
 }
stores-footer-you = あなたはオーナーです。{ -store }には必ずオーナーがいます。
stores-footer-remove-member = ↵ アクセス権を削除
stores-footer-cancel-invite = ↵ この招待を取り消す
stores-footer-show-link = ↵ リンクを表示してコピーと共有
stores-footer-remove-anonymous = ↵ 参加日時しか分からないこの{ -device }を削除
stores-footer-remove-all-anonymous = ↵ リンクから参加したすべての{ -device }を削除
stores-link-copied = ターミナルが対応している場合はクリップボードにコピーしました。そうでない場合は選択してコピーしてください。
stores-link-select = 選択してコピーしてください。
stores-open-link-warning = リンクを持つ人は、名前なしで何度でも参加できます。
stores-open-link-lasts = { -store }が招待のみに戻るまで有効です。
stores-invite-works-once = 一度だけ使えます。{ $expiry }。
stores-invite-preview-safe = ブラウザーで開くと参加方法が表示され、招待は使用されません。
stores-confirm-join = { $name }に参加しますか？
stores-confirm-leave = { $name }から退出しますか？
stores-confirm-remove-member = { $name }を削除しますか？
stores-confirm-cancel-invite = { $name }への招待を取り消しますか？
stores-confirm-unpublish = { $name }の提供を停止しますか？
stores-confirm-open = { $name }のリンクを知っている人が参加できるようにしますか？
stores-confirm-close = { $name }に招待された人だけ参加できるようにしますか？
stores-confirm-remove-device = この{ -device }を削除しますか？
stores-confirm-remove-devices = { $count ->
   *[other] リンクから参加した{ $count }台の{ -device-pl }をすべて削除しますか？
 }
stores-confirm-fallback = 確認
stores-label-address = アドレス
stores-label-key = キー
stores-label-run-by = 運営者
stores-label-device = { -device-cap }
stores-key-matches-link = { $fingerprint }（リンクと一致）
stores-key-matches-invite = { $fingerprint }（招待と一致）
stores-join-open = リンクを知っている人なら誰でも参加できます。
stores-join-anonymous = 名前なしで参加し、同じリンクで別の{ -device }も参加します。
stores-join-already = 参加済みです。もう一度参加すると、この{ -device }に新しいトークンが発行されます。
stores-join-device-note = { -store }のオーナーにこの名前が表示されます。変更するには入力してください。
stores-leave-plugins = この{ -store }からインストールした{ -plugin-cap-pl }は、この端末に残り、更新されなくなります。
stores-leave-revoked = この{ -device }の{ -store }へのアクセス権は取り消されます。
stores-remove-member-devices = { $count ->
   *[other] そのメンバーの{ $count }台の{ -device-pl }は、この{ -store }で直ちに使えなくなります。
 }
stores-remove-member-plugins = すでにインストールされた{ -plugin-cap-pl }は、そのメンバーの端末に残ります。
stores-cancel-invite-link = リンクは使えなくなります。
stores-unpublish-members = インストールしたメンバーは引き続き使えますが、更新されなくなります。
stores-open-anyone = { -store }のリンクを知っている人は、名前なしで参加できます。
stores-open-link-new = リンクは毎回新しく発行されるため、以前のリンクは使えなくなります。
stores-open-members-see = メンバーには、{ -store }がリンクで開かれていることが表示されます。
stores-close-link = リンクは使えなくなり、招待がない限り新しく参加する人はいなくなります。
stores-close-devices = リンクから参加した{ -device-cap-pl }は、メンバー画面で削除するまで使えます。
stores-anonymous-one-stops = この{ -store }で直ちに使えなくなります。
stores-anonymous-all-stop = すべてこの{ -store }で直ちに使えなくなります。
stores-anonymous-plugins = すでにインストールされた{ -plugin-cap-pl }は、そのまま残ります。
stores-anonymous-still-joins = { -store }が開いている間は、リンクから誰でも参加できます。
stores-plugin-may-contact = 接続できるホスト：{ $hosts }
stores-plugin-cannot = ファイルの読み取り、ソケットの開通、環境の確認はできません。
stores-plugin-update-adds = この更新により、接続できるホストに{ $hosts }が追加されます。
stores-plugin-added-by-hand-note = { $id }.wasmは手動で追加されて、{ -plugin }フォルダーにすでにあります。
stores-plugin-other-store-note = 別の{ -store }からインストールされ、そちらで更新されます。
stores-choice-install = インストール
stores-choice-update = { $version }に更新
stores-choice-replace = 手動で追加したものを置き換え
stores-choice-remove = この端末から削除
stores-choice-unpublish = この{ -store }での提供を停止
stores-choice-cancel = キャンセル
stores-publish-new = 新規
stores-publish-published = 公開済み
stores-publish-replaces = { $version }を置き換え
stores-toast-invite-checked = 招待は有効ですが、別の画面が開いています。もう一度貼り付けてください。
stores-toast-nothing-to-publish = 公開できる{ -plugin-pl }が{ -plugin }フォルダーにありません。「{ $command }」を実行して場所を確認してください。
stores-toast-that-is-you = これはあなた自身です。{ -store }には必ずオーナーがいます。
