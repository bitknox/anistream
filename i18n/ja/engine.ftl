notice-saved = { $label }を保存しました。
notice-not-saved = 設定を保存できませんでした。{ $reason }
notice-title-load-failed = この作品の詳細を読み込めませんでした。接続を確認して、もう一度お試しください。
notice-playback-failed = 再生に失敗しました。別の{ -source }をお試しください。
notice-engine-fault = { -app-name }の内部で問題が発生しました。処理は続行されましたが、画面が止まっているように見える場合は、終了してから開き直してください。
notice-database-set-aside = データベースが破損していたため、{ -app-name }は新しいデータベースを作成しました。古いファイルは{ $path }に残されています。{ -tracker-pl }の情報は、次回の同期で戻ります。
start-failed-folder = { -app-name }はフォルダ { $path }を作成できませんでした。ディスクに空きがあり、フォルダに書き込めることを確認してから、もう一度お試しください。{ $reason }
start-failed-config = { $path }に、{ -app-name }では使用できない設定があります: { $problem }。修正するか削除してから、もう一度お試しください。
start-failed-database = { -app-name }は{ $path }のデータベースを開けませんでした。同じフォルダで別の{ -app-name }アプリが開いている場合は、終了してからもう一度お試しください。{ $reason }
start-failed-network = { -app-name }は、config.tomlのネットワーク設定から接続を設定できませんでした。{ $reason }
start-failed = { -app-name }を起動できませんでした。一度終了してから、開き直してください。{ $reason }
start-failed-remote = リモコン機能を開始できませんでした。{ $reason }
start-failed-player = プレーヤーを開始できませんでした。{ $reason }
notice-episodes-queued = { $count ->
   *[other] { $count }件のエピソードをダウンロードキューに追加しました。
 }
