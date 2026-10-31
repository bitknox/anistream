notice-reason = { $reason }
cause-no-sources = Chưa thiết lập { -source-pl }. Hãy thêm một { -source } trong mục { -source-cap-pl }.
notice-not-aired = Tập { $episode } sẽ phát sóng { $when }.
notice-not-aired-yet = Tập { $episode } chưa phát sóng. Tập { $next } sẽ phát sóng { $when }.
cause-airs-today = hôm nay lúc { $time }
cause-airs-tomorrow = ngày mai lúc { $time }
cause-airs-in-days = { $days ->
   *[other] sau { $days } ngày lúc { $time }
 }
cause-airs-on-date = vào { $date } lúc { $time }
cause-no-playable-stream = { -source-cap } đã trả lời nhưng không có gì để phát. Hãy thử { -source } khác.
cause-title-no-match = Không khớp được tựa này: { $detail }
cause-source-gone = { -source-cap } { $source } không còn khả dụng. Hãy chọn một { -source } khác.
cause-release-failed = Không khởi chạy được { -release } đó: { $detail }
cause-sources-timed-out = { -source-cap-pl } phản hồi quá lâu. Hãy thử lại hoặc chọn một { -source } khác.
cause-no-source-has-episode = Không có { -source } nào có tập { $episode }: { $detail }
cause-source-cannot-download = { -source-cap } này phát được tập nhưng không tải về được.
cause-no-source-could-list-show = Không có { -source } nào liệt kê được tựa này, nên không tải về được.
cause-subtitles-merged = { $tracks ->
   *[other] Đã gộp { $tracks } bản phụ đề vào video.
 }
cause-subtitles-left-apart = Phụ đề vẫn nằm trong tệp riêng bên cạnh. { $reason }
cause-already-in-downloads = { $count ->
   *[other] { $count } tập đã có trong mục tải về.
 }
status-looking-up-title = Đang tìm tựa…
status-loading-sources = Đang tải { -source-pl }…
status-searching-sources = Đang tìm trong { -source-pl } tập { $episode }…
notice-player-failed-to-start = Không khởi chạy được mpv. Hãy kiểm tra mpv chạy được khi mở riêng. ({ $reason })
notice-stream-no-video = Luồng này không có hình. Đang thử { $next } trên { $total }.
notice-player-could-not-play = mpv không phát được tệp này. { $reason }
notice-player-exited-silently = mpv đã đóng mà không phát gì và không nêu lý do. Hãy thử { -source } khác.
notice-player-not-playing = mpv chưa bắt đầu phát sau { $seconds } giây. { -source-cap } có thể không có dữ liệu. Hãy kiểm tra mpv có phát được một tệp khi chạy riêng không.
notice-no-player = Phiên bản { -app-name } này không phát được video.
notice-mpv-missing = Không tìm thấy { $binary }. Hãy cài mpv để phát tập.
notice-mpv-old = mpv { $version } có thể phát, nhưng trình phát của { -app-name } cần mpv 0.36 trở lên cho nút bỏ qua, thẻ { -up-next-cap }, menu và ảnh xem trước.
notice-subtitles-unavailable = Không tải được phụ đề của { -source } này. Hãy chọn một { -source } khác để thử lại.
notice-opened-in-browser = Đã mở trong trình duyệt.
notice-could-not-open = Không mở được. { $reason }
notice-trailer-playing = Đang phát trailer trong mpv.
notice-trailer-opened = Đã mở trailer trong trình duyệt.
notice-trailer-failed = Không mở được trailer. { $reason }
notice-filler-skipped = Đã bỏ qua tập filler. { -up-next-cap } là tập { $episode }.
notice-audio-now-subbed = Âm thanh hiện là { -subbed }.
notice-audio-now-dubbed = Âm thanh hiện là { -dubbed }.
notice-already-first-episode = Đây đã là tập đầu tiên.
notice-last-episode = Không có tập sau. Tập { $episode } là tập cuối.
notice-no-next-episode = Không có tập sau.
notice-no-previous-episode = Không còn tập trước.
notice-match-reset-failed = Không đặt lại được kết quả khớp. { $reason }
notice-match-save-failed = Không lưu được kết quả khớp đó. { $reason }
notice-match-did-not-stick = Kết quả khớp đó không được giữ lại. { -source-cap } có thể đã thay đổi. Hãy thử chọn lại.
notice-source-search-empty = Không tìm thấy gì cho “{ $query }”. Hãy thử tìm bằng từ khóa khác.
match-ruled-out-format = Khác định dạng
match-ruled-out-episodes = Nhiều tập hơn tựa này
match-ruled-out-year = Khác năm
match-search-timed-out = Không phản hồi kịp thời.
match-search-source-off = Chưa được bật.
notice-source-pinned = { $source } sẽ được dùng cho tựa này.
notice-source-automatic = Quay lại chọn { -source } tự động.
notice-source-choice-failed = Không lưu được lựa chọn đó. { $reason }
notice-no-source-could-list = Không có { -source } nào liệt kê được tựa này: { $reason }
notice-episodes-failed = Không tải được danh sách tập. { $reason }
notice-match-uncertain = { $source } tìm thấy nhiều hơn một tựa có thể là tựa này, nên tạm thời hiển thị tập từ AniList.
notice-continue-hide-failed = Không xóa được tựa này khỏi { -continue-cap }. { $reason }
notice-download-queued = Đã thêm tập { $episode } của { $title } vào hàng chờ tải.
notice-download-already-queued = Tập { $episode } của { $title } đã có trong mục tải về.
notice-download-starting-now = Đang bắt đầu tải tập { $episode } của { $title }.
notice-download-cancelled = Đã hủy tải về.
notice-download-file-deleted = Đã xóa bản tải về và tệp.
notice-download-removed-file-gone = Đã xóa bản tải về. Tệp của nó đã không còn.
notice-download-removed-file-kept = Đã xóa bản tải về, nhưng không xóa được tệp. { $reason }
notice-download-removed = Đã xóa bản tải về.
notice-downloads-cleared = { $count ->
   *[other] Đã xóa { $count } bản tải về đã xong.
 }
notice-download-finished = Đã tải xong tập { $episode } của { $title }.
notice-download-finished-note = Đã tải xong tập { $episode } của { $title }. { $note }
notice-download-failed = Không tải được tập { $episode } của { $title }. { $reason }
notice-downloads-none-queued = Chưa thêm gì vào hàng chờ.
notice-downloads-none-queued-stopped = Chưa thêm gì vào hàng chờ. { $reason }
notice-downloads-none-new = Chưa thêm gì mới vào hàng chờ. { $existing }
notice-downloads-none-new-stopped = Chưa thêm gì mới vào hàng chờ. { $existing } { $reason }
notice-episodes-queued-existing = { $count ->
   *[other] Đã thêm { $count } tập vào hàng chờ. { $existing }
 }
notice-episodes-queued-stopped = { $count ->
   *[other] Đã thêm { $count } tập vào hàng chờ, sau đó dừng. { $reason }
 }
notice-episodes-queued-existing-stopped = { $count ->
   *[other] Đã thêm { $count } tập vào hàng chờ, sau đó dừng. { $existing } { $reason }
 }
notice-keep-next-full = Không đủ dung lượng trống để giữ các tập sau đã tải. Hãy giải phóng thêm dung lượng để tải thêm.
party-needs-server = Hãy chọn máy chủ cho { -watch-party } trước, dưới tên syncplay.server. { $server } do { $operator } vận hành.
party-choose-server = Hãy chọn máy chủ cho { -watch-party } trước. { $server } do { $operator } vận hành.
notice-no-party = { -watch-party-cap-pl } không có trong phiên bản { -app-name } này.
party-needs-room = Một buổi { -watch-party } cần một phòng để gặp nhau. Hãy chọn phòng trước.
party-server-lost = Mất máy chủ của { -watch-party }. Đang thử lại.
party-rejoined = Đã trở lại { -watch-party }.
party-left = Đã rời { -watch-party }.
