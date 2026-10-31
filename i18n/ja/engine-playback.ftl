notice-reason = { $reason }
cause-no-sources = { -source-pl }が設定されていません。{ -source-cap-pl }の画面で追加してください。
notice-not-aired = 第{ $episode }話は{ $when }に放送予定です。
notice-not-aired-yet = 第{ $episode }話はまだ放送されていません。第{ $next }話は{ $when }に放送予定です。
cause-airs-today = 今日{ $time }
cause-airs-tomorrow = 明日{ $time }
cause-airs-in-days = { $days ->
   *[other] { $days }日後の{ $time }
 }
cause-airs-on-date = { $date }の{ $time }
cause-no-playable-stream = { -source }が応答しましたが、再生できるものがありませんでした。別のものを試してください。
cause-title-no-match = このタイトルと照合できませんでした。{ $detail }
cause-source-gone = { -source }「{ $source }」は利用できなくなりました。別のものを選んでください。
cause-release-failed = その{ -release }を開始できませんでした：{ $detail }
cause-sources-timed-out = { -source-pl }の応答が遅すぎました。もう一度試すか、別の{ -source }を選んでください。
cause-no-source-has-episode = どの{ -source }にも第{ $episode }話がありませんでした：{ $detail }
cause-source-cannot-download = この{ -source }はストリーミングのみで、ダウンロードはできません。
cause-no-source-could-list-show = どの{ -source }もこの作品を一覧に表示できなかったため、ダウンロードできません。
cause-subtitles-merged = { $tracks ->
   *[other] 字幕トラック{ $tracks }件を動画に結合しました。
 }
cause-subtitles-left-apart = 字幕はファイルとして別に残ります。{ $reason }
cause-already-in-downloads = { $count ->
   *[other] { $count }話はすでにダウンロードに入っています。
 }
status-looking-up-title = タイトルを検索しています…
status-loading-sources = { -source-pl }を読み込んでいます…
status-searching-sources = { -source-pl }でエピソード{ $episode }を検索しています…
notice-player-failed-to-start = mpvを起動できませんでした。mpvが単独で起動するか確認してください。（{ $reason }）
notice-stream-no-video = そのストリームから映像が届きませんでした。{ $total }件中{ $next }件目を試しています。
notice-player-could-not-play = mpvでこのファイルを再生できませんでした。{ $reason }
notice-player-exited-silently = mpvは何も再生せず、理由も表示されないまま終了しました。別の{ -source }を試してください。
notice-player-not-playing = { $seconds }秒経っても、mpvで再生が始まりません。{ -source }にデータがない可能性があります。mpvが単独でファイルを再生できるか確認してください。
notice-no-player = この{ -app-name }のバージョンでは動画を再生できません。
notice-mpv-missing = { $binary }が見つかりませんでした。エピソードを再生するには、mpvをインストールしてください。
notice-mpv-old = mpv { $version }では再生できますが、{ -app-name }のプレーヤーは、スキップボタン、{ -up-next-cap }カード、メニュー、サムネイルのためにmpv 0.36以降が必要です。
notice-subtitles-unavailable = この{ -source }の字幕を読み込めませんでした。別の{ -source }を選ぶと、もう一度試せます。
notice-opened-in-browser = ブラウザで開きました。
notice-could-not-open = 開けませんでした。{ $reason }
notice-trailer-playing = mpvで予告編を再生しています。
notice-trailer-opened = ブラウザで予告編を開きました。
notice-trailer-failed = 予告編を開けませんでした。{ $reason }
notice-filler-skipped = フィラーをスキップしました。{ -up-next-cap }はエピソード{ $episode }です。
notice-audio-now-subbed = 音声を{ -subbed }に切り替えました。
notice-audio-now-dubbed = 音声を{ -dubbed }に切り替えました。
notice-already-first-episode = すでに最初のエピソードです。
notice-last-episode = { -up-next-cap }はありません。エピソード{ $episode }が最後です。
notice-no-next-episode = { -up-next-cap }はありません。
notice-no-previous-episode = 前のエピソードはありません。
notice-match-reset-failed = 一致の設定をリセットできませんでした。{ $reason }
notice-match-save-failed = この一致を保存できませんでした。{ $reason }
notice-match-did-not-stick = この一致は反映されませんでした。{ -source }が変わった可能性があります。もう一度選び直してください。
notice-source-search-empty = 「{ $query }」は見つかりませんでした。別の検索語で試してください。
notice-source-pinned = このタイトルには{ $source }を使います。
notice-source-automatic = { -source }の自動選択に戻しました。
notice-source-choice-failed = その選択を保存できませんでした。{ $reason }
notice-no-source-could-list = どの{ -source }もこの作品を一覧に表示できませんでした：{ $reason }
notice-episodes-failed = エピソードを読み込めませんでした。{ $reason }
notice-match-uncertain = { $source }で、この作品の候補になりうる作品が複数見つかりました。候補を選ぶまでは、エピソードはAniListの情報を表示しています。
notice-continue-hide-failed = { -continue-cap }から削除できませんでした。{ $reason }
notice-download-queued = { $title }のエピソード{ $episode }をダウンロードに追加しました。
notice-download-already-queued = { $title }のエピソード{ $episode }はすでにダウンロードに入っています。
notice-download-starting-now = { $title }のエピソード{ $episode }を今すぐ開始します。
notice-download-cancelled = ダウンロードをキャンセルしました。
notice-download-file-deleted = ダウンロードとファイルを削除しました。
notice-download-removed-file-gone = ダウンロードを削除しました。ファイルはすでにありませんでした。
notice-download-removed-file-kept = ダウンロードを削除しましたが、ファイルを削除できませんでした。{ $reason }
notice-download-removed = ダウンロードを削除しました。
notice-downloads-cleared = { $count ->
   *[other] 完了したダウンロードを{ $count }件消去しました。
 }
notice-download-finished = { $title }のエピソード{ $episode }のダウンロードが完了しました。
notice-download-finished-note = { $title }のエピソード{ $episode }のダウンロードが完了しました。{ $note }
notice-download-failed = { $title }のエピソード{ $episode }をダウンロードできませんでした。{ $reason }
notice-downloads-none-queued = 何も追加されませんでした。
notice-downloads-none-queued-stopped = 何も追加されませんでした。{ $reason }
notice-downloads-none-new = 新たに追加されたものはありません。{ $existing }
notice-downloads-none-new-stopped = 新たに追加されたものはありません。{ $existing }{ $reason }
notice-episodes-queued-existing = { $count ->
   *[other] { $count }件のエピソードを追加しました。{ $existing }
 }
notice-episodes-queued-stopped = { $count ->
   *[other] { $count }件のエピソードを追加しましたが、途中で停止しました。{ $reason }
 }
notice-episodes-queued-existing-stopped = { $count ->
   *[other] { $count }件のエピソードを追加しましたが、途中で停止しました。{ $existing }{ $reason }
 }
notice-keep-next-full = { -up-next-cap }を保存するための空き容量が足りません。ダウンロードするには、空き容量を確保してください。
party-needs-server = 先に{ -watch-party }のサーバーを選んでください（syncplay.server）。{ $server }は{ $operator }が運営しています。
party-choose-server = 先に{ -watch-party }のサーバーを選んでください。{ $server }は{ $operator }が運営しています。
notice-no-party = この{ -app-name }のバージョンでは{ -watch-party-cap-pl }は利用できません。
party-needs-room = { -watch-party }に参加する部屋を、先に選んでください。
party-server-lost = { -watch-party }のサーバーから切断されました。再接続しています。
party-rejoined = { -watch-party }に戻りました。
party-left = { -watch-party }から退出しました。
