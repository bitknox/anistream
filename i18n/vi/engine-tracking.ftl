notice-signed-in = Đã đăng nhập vào { $tracker }.
notice-signed-in-keychain = Đã đăng nhập vào { $tracker }. Mã đăng nhập được lưu trong chuỗi khóa của hệ điều hành.
notice-signed-in-file = Đã đăng nhập vào { $tracker }. Không có chuỗi khóa nào, nên mã đăng nhập được lưu trong một tệp riêng chỉ người dùng này đọc được, trên { -device } này.
notice-signed-in-unsaved = Đã đăng nhập vào { $tracker }, nhưng không lưu được mã, nên lần sau bạn sẽ phải đăng nhập lại. { $reason }
notice-token-not-saved = Không lưu được mã đăng nhập. Hãy thử đăng nhập lại. { $reason }
notice-sign-in-failed = Không đăng nhập được vào { $tracker }. Hãy thử lại. { $reason }
notice-sign-in-stopped = Việc đăng nhập vào { $tracker } đã dừng trước khi xong. Hãy thử lại. { $reason }
notice-token-exchange-failed = Không nhận được mã từ { $tracker }. Hãy thử đăng nhập lại. { $reason }
notice-browser-not-opened = Không mở được trình duyệt để đăng nhập. Hãy kiểm tra xem đã cài trình duyệt chưa rồi thử lại. { $reason }
notice-sign-in-not-waiting = Không có phiên đăng nhập vào { $tracker } nào đang chờ phản hồi này. Hãy bắt đầu đăng nhập lại.
notice-sign-in-wrong-request = Phiên đăng nhập vào { $tracker } này không trả về từ yêu cầu đã gửi. Hãy bắt đầu đăng nhập lại.
notice-sign-in-no-token = Phiên đăng nhập vào { $tracker } trả về mà không có mã. Hãy thử lại.
notice-mal-client-missing-build = Không hoàn tất được đăng nhập MyAnimeList: bản dựng này chưa có thành phần kết nối cho dịch vụ đó.
notice-kitsu-signs-in-on-accounts = Kitsu đăng nhập từ màn hình Tài khoản.
notice-sign-in-unsupported = Chưa hỗ trợ đăng nhập vào { $tracker }.
notice-sign-in-needs-redirect = Đăng nhập vào { $tracker } cần chuyển hướng qua trình duyệt, nhưng ứng dụng này không nhận được chuyển hướng đó.
notice-anilist-implicit-flow = Kiểu đăng nhập AniList này chỉ hoạt động trong ứng dụng { -app-name }. Để đăng nhập ở đây, hãy đặt { $setting } thành "loopback" trong tệp cấu hình.
notice-anilist-client-missing = AniList cần client id và secret. Hãy đặt { $client } và { $secret } trong tệp cấu hình, rồi đăng nhập lại. Để lấy địa chỉ đăng nhập, chạy { -app-name } --login-url.
notice-anilist-paste-flow = Không dán được mã ở đây. Hãy thoát rồi chạy { -app-name } --login.
notice-mal-client-missing = MyAnimeList cần client id. Hãy đăng ký một ứng dụng với loại "other" tại myanimelist.net/apiconfig, rồi đặt { $setting } trong tệp cấu hình.
status-waiting-for-redirect = Đang chờ { $tracker } chuyển hướng về…
notice-mal-no-refresh-token = MyAnimeList không cấp mã làm mới. Khoảng một tháng nữa bạn sẽ phải đăng nhập lại.
notice-client-id-missing = Hãy đặt { $setting } trong tệp cấu hình trước.
notice-signed-out = Đã đăng xuất khỏi { $tracker }.
notice-signed-out-queue = { $count ->
   *[other] Đã đăng xuất khỏi { $tracker }. { $count } thay đổi đang chờ được giữ lại cho lần đăng nhập sau.
 }
notice-sign-out-failed = Không đăng xuất được khỏi { $tracker }. Hãy thử lại. { $reason }
notice-not-saved-because = Không lưu được cài đặt đó. { $cause }
notice-protection-saved = Đã lưu cài đặt bảo vệ torrent.
notice-protection-not-checked = Không kiểm tra được bảo vệ torrent. { $cause }
notice-proxy-password-not-kept = Không giữ được mật khẩu proxy. Hãy nhập lại để thử lưu. { $reason }
notice-show-setting-refused = Không lưu được cài đặt đó. { $value } không phải giá trị hợp lệ cho { $setting }.
notice-show-not-reset = Không đặt lại được cài đặt của bộ phim này. { $reason }
notice-show-no-such-subtitles = Không lưu được lựa chọn phụ đề đó. Bộ phim này chưa từng dùng phụ đề đó.
notice-torrents-waiting-for-check = Torrent đang chờ kiểm tra kết nối đạt lại.
notice-mapping-data-refreshed = Đã làm mới dữ liệu ánh xạ.
notice-mapping-data-failed = Không làm mới được dữ liệu ánh xạ ({ $datasets }). Sẽ thử lại sau { $wait }. { $cause }
notice-episode-aired = { $title }, tập { $episode }, đã phát sóng khi bạn vắng mặt.
notice-episodes-aired = { $count ->
   *[other] Có { $count } tập mới của các bộ phim bạn theo dõi đã phát sóng. Lịch chiếu đã có các tập đó.
 }
notice-update-available = Đã có bản { $tag }. Chạy { -app-name } --update để cài đặt.
cause-confirm-no-protection = Hãy xác nhận trước rằng bạn muốn không dùng bảo vệ nào.
cause-confirm-warn-only = Hãy xác nhận trước rằng bạn chỉ muốn được cảnh báo.
cause-proxy-needs-address = Proxy cần có máy chủ và cổng.
cause-proxy-address-unusable = Không kiểm tra được địa chỉ proxy này.
cause-server-unreachable = Không kết nối được tới máy chủ.
