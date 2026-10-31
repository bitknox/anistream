episode-short-dubbed = E{ $episode } · { -dubbed-cap }
episode-long-dubbed = エピソード { $episode } · { -dubbed-cap }
downloads-live-paused = { $count ->
   *[other] { $count }エピソードが一時停止中
 }
downloads-live-lead = { $title } E{ $episode }
downloads-live-left = 残り { $size }
downloads-live-progress = { $done } / { $total }
downloads-live-paused-one = 一時停止中
downloads-live-waiting = 待機中
settings-seed-ratio-until = 比率 { $ratio } まで
settings-upload-rate = { $size }/秒
settings-coming-up-dubbed = { $count ->
   *[other] { $names } は{ -dubbed }が出たら通知します。
 }
settings-coming-up-switched-off = { $count ->
   *[other] { $names } は、各作品の設定で通知がオフになっています。
 }
settings-coming-up-opens = 作品の設定を開きます
torrents-check-note-warn = { -app-name } は、トレントを開始したときと実行中は毎分、トレントの通信がどこから出ているかを確認します。確認に失敗した場合は警告します。VPNまたはプロキシが通信を保護します。
torrents-check-note-stop = { -app-name } は、トレントを開始したときと実行中は毎分、トレントの通信がどこから出ているかを確認します。確認に失敗した場合はトレントを停止します。VPNまたはプロキシが通信を保護します。
torrents-protect-proxy-name = SOCKS5プロキシ
torrents-protect-vpn-name = この{ -device }のVPN
torrents-warn-body = { -app-name } は一時停止せずに警告します。VPNまたはプロキシが停止していると、他の人にIPアドレスを見られます。
torrents-proxy-as = { $user } として
title-facts-aired = { $dates }放送
title-facts-airing = { $date }から放送中
title-facts-airs = { $date }から放送予定
media-dates-range = { $from }〜{ $to }
title-also-on-more = { $more ->
   *[other] { $first } ほか{ $more }件
 }
title-adult-mark = 18+
downloads-storage-used-free = { $used } 使用中 · 空き { $free }
downloads-storage-used = { $used } 使用中
downloads-time-left = 残り { $time }
downloads-rate = { $size }/秒
downloads-uploaded = { $size } 送信済み
downloads-ratio-of = { $ratio } / { $goal }
downloads-group-summary = { $count ->
   *[other] { $count }エピソード · { $size }
 }
episode-menu-download = ダウンロード
episode-menu-download-paused = ダウンロードを一時停止中
reason-timeout-anilist = AniListからの応答に時間がかかりすぎました。
reason-timeout-server = サーバーからの応答に時間がかかりすぎました。
reason-busy-anilist = AniListが混み合っています。1分ほど待ってから、もう一度お試しください。
reason-busy-server = サーバーが混み合っています。1分ほど待ってから、もう一度お試しください。
reason-problem-anilist = AniListで問題が発生しました。
reason-problem-server = サーバーで問題が発生しました。
reason-silent-anilist = AniListからの応答がありませんでした。
reason-silent-server = サーバーからの応答がありませんでした。
updates-installing-plain = インストール中
updates-closes-as-installs = インストール中に{ -app-name }は終了します。
updates-open-release = ページを開く
country-korea = 韓国
search-genres-picked = { $count ->
   *[other] { $count }ジャンル
 }
search-formats-picked = { $count ->
   *[other] { $count }形式
 }
search-statuses-picked = { $count ->
   *[other] { $count }ステータス
 }
filter-score-any = 指定なし
filter-years-from = { $year }年以降
filter-years-between = { $from }〜{ $to }
filter-tag-rank-title = タグの順位の下限
filter-tag = タグ
search-filter-by = { $name }で絞り込む
search-loading-genres = ジャンルを読み込んでいます
search-paused-rate-limit = 入力中の自動検索は、AniListの利用制限を超えないよう一時停止しています。今すぐ検索するには、検索キーを押してください。
action-clear-all = すべて解除
filter-source-material = 原作
search-detail-episode = { $status } · E{ $episode }
sources-use = { $name } を使う
sources-move = { $name } を移動
sources-reorder = 並べ替え
sources-version-available = { $version } 利用可能
sources-update-from = { $store }のバージョン { $version }があります。
sources-detail-off = オフです。順番は保たれますが、オンに戻すまで試されません。
sources-fallback-title = { -source-cap }
reason-source-timeout = 応答に時間がかかりすぎました。
reason-source-busy = 混み合っています。1分ほど待ってから、もう一度お試しください。
reason-source-problem = 問題が発生しました。
reason-source-turned-away = リクエストが拒否されました。
reason-source-no-address = アドレスが見つかりませんでした。
reason-source-unreachable = 接続できませんでした。
stores-updates-waiting = { $count ->
   *[other] { $count }件の更新
 }
stores-checked-ago = { $ago }に確認
stores-preview-key-link = キーがリンクと一致します
stores-preview-key-invite = キーが招待と一致します
stores-key-verified = 確認済み
stores-fallback-title = { -plugin-store-cap }
stores-fallback-plugin = { -plugin-cap }
stores-fallback-name = { -plugin-store }
stores-link-sheet-detail = 別の{ -device }でこのリンクを開くと、そこで{ -plugin-store }に参加できます。リンクは一度だけ使えます。
accounts-synced-ago = { $ago }に同期済み
accounts-card-line = { $tracker } · { $status }
accounts-card-description = { $name }、{ $line }。アカウントを開きます
accounts-card-description-signed-out = サインインしていません。アカウントを開きます
accounts-signed-in-plain = サインイン済み
settings-summary-notify-on = 新着エピソード オン
settings-summary-notify-off = 新着エピソード オフ
settings-summary-sources = { $count ->
   *[other] { $count }{ -source-pl }
 }
settings-summary-no-sources = まだ{ -source-pl }はありません
settings-summary-torrents-on = トレント オン
settings-summary-torrents-off = トレント オフ
library-storage-on-device = この{ -device }に{ $size }
library-storage-none = この{ -device }に保存されたものはありません
library-detail-of = E{ $next } / { $total }
library-detail-next = E{ $next }
library-sort-last-watched = 最終視聴日
library-sort-current = 現在
settings-notify-permission-note = { -app-name }は、この設定を初めてオンにするときに許可を求めます。
settings-notify-refused = オフ: Androidで{ -app-name }の通知が許可されていません
settings-notify-system-title = 通知の設定
settings-notify-system-detail = サウンド、バイブレーション、ロック画面
settings-titles-line-signed-out = AniListからサインアウトしている間は英語
settings-titles-line-account-language = { $language }、AniListアカウントの設定から
settings-titles-line-follows = AniListアカウントの設定に従う
settings-titles-line-english = 英語がない場合はローマ字
settings-titles-line-romaji = 日本語のタイトルをローマ字で表示
settings-titles-note-account = AniListアカウントの設定に従います。アカウントの言語は{ $language }です。AniListからサインアウトしている間は、タイトルは英語で表示されます。
settings-titles-note-follows = AniListアカウントの設定に従います。AniListからサインアウトしている間は、タイトルは英語で表示されます。
settings-titles-note-english-missing = { $title }にはAniListで英語のタイトルがないため、ローマ字のタイトルのままです。
settings-titles-note-english = AniListに登録されている英語のタイトルを表示します。英語がない場合はローマ字を表示します。
settings-titles-note-romaji = AniListに登録されている日本語のタイトルを、ローマ字で表示します。
settings-index-none = 未設定
settings-stores-with-updates = { $stores }件 · { $updates }
settings-seeding-best-effort = この{ -device }ではベストエフォートです。シーディングは{ -app-name }を開いている間だけ行われます。
settings-downloads-count = { $count ->
   *[other] { $count }エピソード
 }
settings-anilist-footer = 進捗とリストの変更はAniListに同期されます。{ -library-cap }では、AniListのリストを表示します。
accounts-sign-in-again-button = 再度サインイン
accounts-review-differences = { $count ->
   *[other] { $count }件の差分を確認
 }
settings-licence-title = ライセンス
settings-licence-value = MIT、オープンソース、非商用
settings-typefaces-value = Roboto Flex と Zen Kaku Gothic New。SIL Open Font License 1.1 のもとで提供
settings-libraries-value = JNA、AndroidX（Apache License 2.0）
settings-report-bug-value = ブラウザでGitHubを開きます
downloads-storage-share-description = ダウンロードが { $size } を使用しています
settings-aside-notifications = フォローしている作品に新しいエピソードが公開されると、通知が1件届きます。通知には作品名と話数が含まれ、エピソードのタイトルは含まれません。作品ごとの設定で通知をオフにできます。
settings-aside-downloads = ダウンロードは、この{ -device }の{ -app-name }専用の領域に保存され、他のアプリからは読み取れません。アプリを削除すると、ダウンロードも削除されます。
settings-aside-connection-check = 接続の確認
settings-aside-sources = { -source-cap-pl }は上から順番に試されます。作品ごとに、作品ページで選んだ{ -source }を使うこともできます。
settings-upscaling-description = 同じ静止画です。左半分は元の480p、右半分は{ $mode }です。
settings-upscaling-source = 元の480p
settings-upscaling-note-off-original = 元の480pを、加工せずに表示サイズに合わせています。
torrents-sheet-turn-on-title = トレントをオンにする
torrents-sheet-protect-title = トレント通信の保護
torrents-sheet-turn-on-button = オンにする
torrents-sheet-host = ホスト
torrents-sheet-port = ポート
torrents-sheet-proxy-auth-note = プロキシがユーザー名とパスワードを求める場合のみ入力してください。
torrents-sheet-edit-proxy-title = プロキシを編集
title-facts-also-on = ほかでも視聴可能
title-fetch-failed-detail = AniListからの応答がありませんでした。
continue-resize-queue = キューのサイズを変更
continue-go-to-title = 作品を開く
continue-short-left = { $episode } · 残り { $left }
continue-airing-out = E{ $episode } · { $time } · { $ago }前に公開
continue-airing-in = E{ $episode } · { $time } · あと{ $countdown }で公開
calendar-line-out = { $episode } · { $ago }前に公開
calendar-line-in = { $episode } · あと{ $countdown }で公開
nav-accounts-and-settings-user = { $user }、アカウントと設定
episode-unaired-label = エピソード { $number }、{ $title }。まだ公開されていません
episode-unaired-label-plain = エピソード { $number }。まだ公開されていません
episode-unaired-airs-label = エピソード { $number }、{ $title }。{ $airs }に公開予定です
episode-unaired-airs-label-plain = エピソード { $number }。{ $airs }に公開予定です
tv-sheet-on-network = ネットワーク上のテレビ
tv-sheet-not-listed = 表示されない場合
tv-sheet-scan = テレビ上のコードをスキャン
tv-sheet-code-on = { $name }に表示されたコード
tv-sheet-pairing-button = ペアリング中
tv-sheet-connecting = テレビに接続しています
tv-sheet-type-code = コードを入力
tv-sheet-volume = テレビの音量
tv-sheet-volume-down = 音量を下げる
tv-sheet-volume-up = 音量を上げる
tv-sheet-volume-value = { $value } / 100
tv-sheet-nothing-playing = 再生していません
tv-sheet-position = テレビでの再生位置
tv-sheet-position-state = { $at } / { $total }
tv-sheet-request-titled = { $title }、エピソード { $episode }
tv-sheet-request-plain = エピソード { $episode }
tv-sheet-footer = テレビには、この{ -device }がコード { $code } で表示されます。ここでテレビの登録を解除すると、テレビ側でもこの{ -device }の登録が解除されます。
tv-sheet-title-fallback = テレビ
episode-line-detail = E{ $number } · { $detail }
calendar-day-label = { $day }
calendar-day-label-today = { $day }、今日
calendar-day-label-airing = { $day }、公開あり
calendar-day-label-today-airing = { $day }、今日、公開あり
continue-show-airing-in-calendar = 今日公開の作品をカレンダーで表示
continue-show-season-in-seasonal = 今期の作品を今期一覧で表示
search-searching-filters = { $filters } を検索中
episode-number-plain = エピソード { $number }
episode-number-title-text = エピソード { $number } · { $title }
player-position-state = { $at } / { $total }
player-sheet-finding = { -source-pl }を検索中…
player-sheet-none = このエピソードの{ -source-pl }は見つかりませんでした。
player-sheet-episodes-loading = エピソード一覧を読み込んでいます。
player-dual-audio = { -subbed-cap }と{ -dubbed }
notif-new-episodes = { $count ->
   *[other] 新しいエピソードが{ $count }件
 }
notif-hidden-text = 新しいエピソード
notif-episode-out = エピソード { $episode } が公開されました。
notif-episode-out-dubbed = エピソード { $episode } が{ -dubbed }で公開されました。
notif-episode-out-next = エピソード { $episode } が公開されました。次はエピソード { $next } です。
notif-episode-out-dubbed-next = エピソード { $episode } が{ -dubbed }で公開されました。次はエピソード { $next } です。
device-name-phone = Androidスマートフォン
device-name-tablet = Androidタブレット
tv-session-starting = { $tv }で再生を開始しています
tv-session-line = { $line } · { $tv }
widget-line-left = { $episode } · 残り{ $minutes }分
widget-line-of = E{ $episode } / { $total }
widget-continue-title = { -continue-cap }
widget-caught-up-title = 最新まで視聴済み
widget-caught-up = 最新のエピソードまで視聴済みです。
widget-caught-up-next = { $title } E{ $episode } は { $airs } に公開予定です。
widget-day-date = { $day }
widget-play-dubbed-label = エピソード { $episode }、{ -dubbed }、{ $title } を再生
tv-action-resume = エピソード { $episode } を再開
tv-action-resume-dubbed = エピソード { $episode } · { -dubbed-cap } を再開
tv-action-out = エピソード { $episode } を再生（{ $day }公開）
tv-action-not-out = エピソード { $episode } を再生（まだ公開されていません）
tv-out-today = 今日
tv-hero-resume = { $episode } を再開
tv-hero-start = { $episode } を再生
