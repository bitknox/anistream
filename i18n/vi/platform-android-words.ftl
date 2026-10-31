episode-short-dubbed = E{ $episode } · { -dubbed-cap }
episode-long-dubbed = Tập { $episode } · { -dubbed-cap }
downloads-live-paused = { $count ->
   *[other] { $count } tập đã tạm dừng
 }
downloads-live-lead = { $title } E{ $episode }
downloads-live-left = Còn { $size }
downloads-live-progress = { $done } trên { $total }
downloads-live-paused-one = Đã tạm dừng
downloads-live-waiting = Đang chờ
settings-seed-ratio-until = Đến tỷ lệ { $ratio }
settings-upload-rate = { $size }/s
settings-coming-up-dubbed = { $count ->
   *[other] { $names } sẽ thông báo khi bản { -dubbed } ra mắt.
 }
settings-coming-up-switched-off = { $count ->
   *[other] { $names } đang tắt trong cài đặt riêng của tựa.
 }
settings-coming-up-opens = Chạm vào một tựa để mở cài đặt riêng của tựa đó.
torrents-check-note-warn = { -app-name } kiểm tra nơi lưu lượng torrent đi ra khi một torrent bắt đầu và mỗi phút trong lúc nó chạy, rồi cảnh báo bạn nếu kiểm tra thất bại. VPN hoặc proxy của bạn mới là thứ bảo vệ bạn.
torrents-check-note-stop = { -app-name } kiểm tra nơi lưu lượng torrent đi ra khi một torrent bắt đầu và mỗi phút trong lúc nó chạy, rồi dừng torrent nếu kiểm tra thất bại. VPN hoặc proxy của bạn mới là thứ bảo vệ bạn.
torrents-protect-proxy-name = Proxy SOCKS5
torrents-protect-vpn-name = VPN trên { -device } này
torrents-warn-body = { -app-name } sẽ cảnh báo bạn thay vì tạm dừng. Nếu VPN hoặc proxy của bạn ngừng hoạt động, người khác có thể thấy địa chỉ IP của bạn.
torrents-proxy-as = với tên { $user }
title-facts-aired = Phát sóng { $dates }
title-facts-airing = Đang phát sóng từ { $date }
title-facts-airs = Sẽ phát sóng từ { $date }
media-dates-range = { $from } đến { $to }
title-also-on-more = { $more ->
   *[other] { $first } và { $more } nơi khác
 }
title-adult-mark = 18+
downloads-storage-used-free = { $used } đã dùng · { $free } còn trống
downloads-storage-used = { $used } đã dùng
downloads-time-left = Còn { $time }
downloads-rate = { $size }/s
downloads-uploaded = Đã tải lên { $size }
downloads-ratio-of = { $ratio } trên { $goal }
downloads-group-summary = { $count ->
   *[other] { $count } tập · { $size }
 }
episode-menu-download = Tải xuống
episode-menu-download-paused = Đã tạm dừng tải xuống
reason-timeout-anilist = AniList phản hồi quá lâu.
reason-timeout-server = Máy chủ phản hồi quá lâu.
reason-busy-anilist = AniList đang bận. Thử lại sau một phút.
reason-busy-server = Máy chủ đang bận. Thử lại sau một phút.
reason-problem-anilist = AniList gặp sự cố.
reason-problem-server = Máy chủ gặp sự cố.
reason-silent-anilist = AniList không phản hồi.
reason-silent-server = Máy chủ không phản hồi.
updates-installing-plain = Đang cài đặt
updates-closes-as-installs = { -app-name } đóng lại trong lúc cài đặt.
updates-open-release = Mở trang
country-korea = Hàn Quốc
search-genres-picked = { $count ->
   *[other] { $count } thể loại
 }
search-formats-picked = { $count ->
   *[other] { $count } định dạng
 }
search-statuses-picked = { $count ->
   *[other] { $count } trạng thái
 }
filter-score-any = Bất kỳ
filter-years-from = Từ { $year }
filter-years-between = { $from } đến { $to }
filter-tag-rank-title = Xếp hạng thẻ tối thiểu
filter-tag = Thẻ
search-filter-by = Lọc theo { $name }
search-loading-genres = Đang tải thể loại
search-paused-rate-limit = Tìm kiếm khi gõ đang tạm dừng một lát để không vượt giới hạn của AniList. Nhấn phím tìm kiếm để tìm ngay.
action-clear-all = Xóa tất cả
filter-source-material = Dựa trên
search-detail-episode = { $status } · E{ $episode }
sources-use = Dùng { $name }
sources-move = Di chuyển { $name }
sources-reorder = Sắp xếp lại
sources-version-available = Có bản { $version }
sources-update-from = Phiên bản { $version } từ { $store }.
sources-detail-off = Đã tắt. { -source-cap } giữ nguyên vị trí và không được thử cho đến khi bật lại.
sources-fallback-title = { -source-cap }
reason-source-timeout = Quá thời gian chờ phản hồi.
reason-source-busy = Đang bận. Thử lại sau một phút.
reason-source-problem = Đã gặp sự cố.
reason-source-turned-away = Đã từ chối yêu cầu.
reason-source-no-address = Không tìm thấy địa chỉ.
reason-source-unreachable = Không kết nối được.
stores-updates-waiting = { $count ->
   *[other] { $count } bản cập nhật
 }
stores-checked-ago = kiểm tra { $ago }
stores-preview-key-link = Khóa khớp với liên kết
stores-preview-key-invite = Khóa khớp với lời mời
stores-key-verified = Đã xác minh
stores-fallback-title = { -plugin-store-cap }
stores-fallback-plugin = { -plugin-cap }
stores-fallback-name = { -plugin-store }
stores-link-sheet-detail = Mở liên kết này trên { -device } khác của bạn để tham gia { -plugin-store } ở đó. Liên kết chỉ dùng được một lần.
accounts-synced-ago = Đã đồng bộ { $ago }
accounts-card-line = { $tracker } · { $status }
accounts-card-description = { $name }, { $line }. Mở Tài khoản
accounts-card-description-signed-out = Chưa đăng nhập. Mở Tài khoản
accounts-signed-in-plain = Đã đăng nhập
settings-summary-notify-on = Tập mới đang bật
settings-summary-notify-off = Tập mới đang tắt
settings-summary-sources = { $count ->
   *[other] { $count } { -source }
 }
settings-summary-no-sources = Chưa có { -source-pl }
settings-summary-torrents-on = torrent đang bật
settings-summary-torrents-off = torrent đang tắt
library-storage-on-device = { $size } trên { -device } này
library-storage-none = Không có gì trên { -device } này
library-detail-of = E{ $next } trên { $total }
library-detail-next = E{ $next }
library-sort-last-watched = Xem gần nhất
library-sort-current = Đang dùng
settings-notify-permission-note = { -app-name } sẽ xin quyền lần đầu bạn bật mục này.
settings-notify-refused = Đang tắt: Android chưa cho phép { -app-name } gửi thông báo.
settings-notify-system-title = Cài đặt thông báo
settings-notify-system-detail = Âm thanh, rung và màn hình khóa
settings-titles-line-signed-out = Tiếng Anh khi chưa đăng nhập AniList
settings-titles-line-account-language = { $language }, theo tài khoản AniList của bạn
settings-titles-line-follows = Theo tài khoản AniList của bạn
settings-titles-line-english = Romaji nếu không có tên tiếng Anh
settings-titles-line-romaji = Tên tiếng Nhật viết bằng chữ Latin
settings-titles-note-account = Theo tài khoản AniList của bạn, đang đặt là { $language }. Khi chưa đăng nhập AniList, tựa sẽ hiển thị bằng tiếng Anh.
settings-titles-note-follows = Theo tài khoản AniList của bạn. Khi chưa đăng nhập AniList, tựa sẽ hiển thị bằng tiếng Anh.
settings-titles-note-english-missing = { $title } không có tên tiếng Anh trên AniList, nên vẫn dùng tên romaji.
settings-titles-note-english = Tên tiếng Anh như AniList ghi, và romaji nếu không có tên tiếng Anh.
settings-titles-note-romaji = Tên tiếng Nhật viết bằng chữ Latin, như AniList ghi.
settings-index-none = Chưa đặt
settings-stores-with-updates = { $stores } · { $updates }
settings-seeding-best-effort = Cố gắng hết sức trên { -device } này: seed chỉ chạy khi { -app-name } đang mở.
settings-downloads-count = { $count ->
   *[other] { $count } tập
 }
settings-anilist-footer = Tiến độ và thay đổi danh sách được đồng bộ sang AniList. { -library-cap } hiển thị các danh sách AniList của bạn.
accounts-sign-in-again-button = Đăng nhập lại
accounts-review-differences = { $count ->
   *[other] Xem { $count } khác biệt
 }
settings-licence-title = Giấy phép
settings-licence-value = MIT, mã nguồn mở và phi thương mại
settings-typefaces-value = Roboto Flex và Zen Kaku Gothic New, theo SIL Open Font License 1.1
settings-libraries-value = JNA và AndroidX, theo Apache License 2.0
settings-report-bug-value = Mở GitHub trong trình duyệt của bạn
downloads-storage-share-description = Tải xuống chiếm { $size }
settings-aside-notifications = Một thông báo khi có tập mới của tựa bạn theo dõi: tên tựa và số tập, không có tên tập. Mỗi tựa có thể tự tắt thông báo trong cài đặt của nó.
settings-aside-downloads = Bản tải xuống nằm trong không gian riêng của { -app-name } trên { -device } này, ứng dụng khác không đọc được. Gỡ ứng dụng sẽ xóa chúng.
settings-aside-connection-check = Kiểm tra kết nối
settings-aside-sources = { -source-cap-pl } được thử theo thứ tự từ trên xuống dưới. Một tựa có thể giữ { -source } riêng, chọn trong trang tựa.
settings-upscaling-description = Cùng một khung hình: nửa trái là 480p gốc, nửa phải là { $mode }
settings-upscaling-source = 480p gốc
settings-upscaling-note-off-original = 480p gốc, được phóng để vừa mà không xử lý.
torrents-sheet-turn-on-title = Bật torrent
torrents-sheet-protect-title = Bảo vệ lưu lượng torrent
torrents-sheet-turn-on-button = Bật
torrents-sheet-host = Máy chủ
torrents-sheet-port = Cổng
torrents-sheet-proxy-auth-note = Tên người dùng và mật khẩu chỉ cần khi proxy yêu cầu.
torrents-sheet-edit-proxy-title = Sửa proxy
title-facts-also-on = Cũng có trên
title-fetch-failed-detail = AniList không phản hồi.
continue-resize-queue = Đổi kích thước hàng đợi
continue-go-to-title = Đến trang tựa
continue-short-left = { $episode } · còn { $left }
continue-airing-out = E{ $episode } · { $time } · đã ra { $ago }
continue-airing-in = E{ $episode } · { $time } · trong { $countdown }
calendar-line-out = { $episode } đã ra { $ago }
calendar-line-in = { $episode } trong { $countdown }
nav-accounts-and-settings-user = { $user }, tài khoản và cài đặt
episode-unaired-label = Tập { $number }, { $title }. Chưa ra mắt
episode-unaired-label-plain = Tập { $number }. Chưa ra mắt
episode-unaired-airs-label = Tập { $number }, { $title }. Sẽ phát sóng { $airs }, chưa ra mắt
episode-unaired-airs-label-plain = Tập { $number }. Sẽ phát sóng { $airs }, chưa ra mắt
tv-sheet-on-network = Trên mạng này
tv-sheet-not-listed = Không có trong danh sách
tv-sheet-scan = Quét mã trên TV
tv-sheet-code-on = Mã trên { $name }
tv-sheet-pairing-button = Đang ghép đôi
tv-sheet-connecting = Đang kết nối tới TV
tv-sheet-type-code = Nhập mã
tv-sheet-volume = Âm lượng TV
tv-sheet-volume-down = Giảm âm lượng
tv-sheet-volume-up = Tăng âm lượng
tv-sheet-volume-value = { $value } trên 100
tv-sheet-nothing-playing = Không phát gì
tv-sheet-position = Vị trí trên TV
tv-sheet-position-state = { $at } trên { $total }
tv-sheet-request-titled = { $title }, tập { $episode }
tv-sheet-request-plain = Tập { $episode }
tv-sheet-footer = TV sẽ thấy { -device } này với mã { $code }. Quên TV ở đây cũng sẽ quên { -device } này.
tv-sheet-title-fallback = TV
episode-line-detail = E{ $number } · { $detail }
calendar-day-label = { $day }
calendar-day-label-today = { $day }, hôm nay
calendar-day-label-airing = { $day }, có tập phát sóng
calendar-day-label-today-airing = { $day }, hôm nay, có tập phát sóng
continue-show-airing-in-calendar = Xem tựa phát sóng hôm nay trong Lịch
continue-show-season-in-seasonal = Xem tựa mùa này trong Theo mùa
search-searching-filters = Đang tìm { $filters }
episode-number-plain = Tập { $number }
episode-number-title-text = Tập { $number } · { $title }
player-position-state = { $at } trên { $total }
player-sheet-finding = Đang tìm { -source-pl }…
player-sheet-none = Không tìm thấy { -source-pl } cho tập này.
player-sheet-episodes-loading = Danh sách tập đang được tải.
player-dual-audio = { -subbed-cap } và { -dubbed }
notif-new-episodes = { $count ->
   *[other] { $count } tập mới
 }
notif-hidden-text = Tập mới
notif-episode-out = Tập { $episode } đã ra mắt.
notif-episode-out-dubbed = Tập { $episode } đã ra, { -dubbed }.
notif-episode-out-next = Tập { $episode } đã ra mắt. { -up-next-cap } cho bạn: tập { $next }.
notif-episode-out-dubbed-next = Tập { $episode } đã ra, { -dubbed }. { -up-next-cap } cho bạn: tập { $next }.
device-name-phone = Điện thoại Android
device-name-tablet = Máy tính bảng Android
tv-session-starting = Đang bắt đầu trên { $tv }
tv-session-line = { $line } · { $tv }
widget-line-left = { $episode } · còn { $minutes } phút
widget-line-of = E{ $episode } trên { $total }
widget-continue-title = { -continue-cap }
widget-caught-up-title = Đã theo kịp
widget-caught-up = Bạn đã theo kịp.
widget-caught-up-next = { $title } E{ $episode } phát sóng { $airs }.
widget-day-date = { $day }
widget-play-dubbed-label = Phát tập { $episode }, { -dubbed }, của { $title }
tv-action-resume = { -continue-cap } tập { $episode }
tv-action-resume-dubbed = { -continue-cap } tập { $episode } · { -dubbed-cap }
tv-action-out = Phát tập { $episode } (ra mắt { $day })
tv-action-not-out = Phát tập { $episode } (chưa ra mắt)
tv-out-today = hôm nay
tv-hero-resume = { -continue-cap } { $episode }
tv-hero-start = Phát { $episode }
