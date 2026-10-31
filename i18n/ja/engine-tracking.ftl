notice-signed-in = { $tracker }にサインインしました。
notice-signed-in-keychain = { $tracker }にサインインしました。トークンはOSのキーチェーンに保存されています。
notice-signed-in-file = { $tracker }にサインインしました。キーチェーンが利用できないため、トークンはこの{ -device }内の非公開ファイルに保存されています。
notice-signed-in-unsaved = { $tracker }にサインインしましたが、トークンを保存できなかったため、次回は再度サインインが必要です。{ $reason }
notice-token-not-saved = サインイントークンを保存できませんでした。もう一度サインインしてください。{ $reason }
notice-sign-in-failed = { $tracker }にサインインできませんでした。もう一度試してください。{ $reason }
notice-sign-in-stopped = { $tracker }へのサインインが完了する前に中断されました。もう一度試してください。{ $reason }
notice-token-exchange-failed = { $tracker }からトークンを取得できませんでした。もう一度サインインしてください。{ $reason }
notice-browser-not-opened = サインイン用のブラウザを開けませんでした。ブラウザがインストールされているか確認して、もう一度試してください。{ $reason }
notice-sign-in-not-waiting = 待機中の{ $tracker }のサインインはありません。サインインを最初からやり直してください。
notice-sign-in-wrong-request = { $tracker }から戻ったサインイン情報が、今回の要求のものではありません。サインインを最初からやり直してください。
notice-sign-in-no-token = { $tracker }へのサインインがトークンなしで戻ってきました。もう一度試してください。
notice-mal-client-missing-build = MyAnimeListへのサインインを完了できませんでした。このビルドにはMyAnimeList用のクライアントが含まれていません。
notice-kitsu-signs-in-on-accounts = Kitsuへのサインインは、アカウント画面から行います。
notice-sign-in-unsupported = { $tracker }へのサインインは、まだ対応していません。
notice-sign-in-needs-redirect = { $tracker }へのサインインには、このアプリでは受け取れないブラウザのリダイレクトが必要です。
notice-anilist-implicit-flow = このAniListのサインイン方法は、{ -app-name }アプリでのみ使えます。ここでサインインするには、設定ファイルで{ $setting }を「loopback」にしてください。
notice-anilist-client-missing = AniListにはクライアントIDとシークレットが必要です。設定ファイルで{ $client }と{ $secret }を設定してから、もう一度サインインしてください。サインイン用のアドレスは、{ -app-name } --login-urlを実行すると表示されます。
notice-anilist-paste-flow = ここではコードを貼り付けてサインインできません。終了して、代わりに{ -app-name } --loginを実行してください。
notice-mal-client-missing = MyAnimeListにはクライアントIDが必要です。myanimelist.net/apiconfigで種類を「other」としてアプリを登録し、設定ファイルで{ $setting }を設定してください。
status-waiting-for-redirect = { $tracker }がリダイレクトで戻るのを待っています…
notice-mal-no-refresh-token = MyAnimeListはリフレッシュトークンを発行しませんでした。約1か月後に、再度サインインが必要になります。
notice-client-id-missing = 先に設定ファイルで{ $setting }を設定してください。
notice-signed-out = { $tracker }からサインアウトしました。
notice-signed-out-queue = { $count ->
   *[other] { $tracker }からサインアウトしました。送信待ちの変更が{ $count }件あり、次回のサインイン後に送信されます。
 }
notice-sign-out-failed = { $tracker }からサインアウトできませんでした。もう一度試してください。{ $reason }
notice-not-saved-because = その設定を保存できませんでした。{ $cause }
notice-protection-saved = トレントの保護設定を保存しました。
notice-protection-not-checked = トレントの保護を確認できませんでした。{ $cause }
notice-proxy-password-not-kept = プロキシのパスワードを保存できませんでした。もう一度入力して、保存をお試しください。{ $reason }
notice-show-setting-refused = その設定を保存できませんでした。{ $value }は{ $setting }に使えない値です。
notice-show-not-reset = この作品の設定をリセットできませんでした。{ $reason }
notice-show-no-such-subtitles = その字幕の選択を保存できませんでした。この作品では、その字幕が使われたことがありません。
notice-torrents-waiting-for-check = トレントは、接続の確認が再び通るまで待機しています。
notice-mapping-data-refreshed = マッピングデータを更新しました。
notice-mapping-data-failed = マッピングデータを更新できませんでした（{ $datasets }）。{ $wait }後に再試行します。{ $cause }
notice-episode-aired = { $title }のエピソード{ $episode }が、離れている間に放送されました。
notice-episodes-aired = { $count ->
   *[other] 視聴中の作品の新しいエピソードが{ $count }件放送されました。カレンダーで確認できます。
 }
notice-update-available = { $tag }が公開されました。{ -app-name } --updateを実行するとインストールできます。
cause-confirm-no-protection = 先に、保護なしで問題ないことを確認してください。
cause-confirm-warn-only = 先に、警告のみにすることを確認してください。
cause-proxy-needs-address = プロキシにはホストとポートが必要です。
cause-proxy-address-unusable = プロキシのアドレスは、確認に使えないアドレスです。
cause-server-unreachable = サーバーに接続できませんでした。
