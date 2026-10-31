notice-saved = Đã lưu { $label }.
notice-not-saved = Không lưu được cài đặt đó. { $reason }
notice-title-load-failed = Không tải được chi tiết của bộ phim này. Hãy kiểm tra kết nối rồi thử lại.
notice-playback-failed = Không phát được. Hãy thử { -source } khác.
notice-engine-fault = Có lỗi xảy ra bên trong { -app-name }. Ứng dụng vẫn chạy tiếp, nhưng nếu màn hình có vẻ bị treo, hãy thoát rồi mở lại.
notice-database-set-aside = Cơ sở dữ liệu bị hỏng, nên { -app-name } đã tạo một cơ sở dữ liệu mới. Tệp cũ được giữ tại { $path }. { -tracker-pl } sẽ khôi phục dữ liệu mà chúng đang giữ ở lần đồng bộ sau.
start-failed-folder = { -app-name } không tạo được thư mục { $path }. Hãy kiểm tra xem ổ đĩa còn chỗ trống và thư mục có ghi được không, rồi thử lại. { $reason }
start-failed-config = { $path } có một cài đặt mà { -app-name } không dùng được: { $problem }. Hãy sửa hoặc xóa cài đặt đó, rồi thử lại.
config-reset-failed = { -app-name } không di chuyển được { $path } đi chỗ khác để đặt lại cài đặt. { $reason }
start-failed-database = { -app-name } không mở được cơ sở dữ liệu tại { $path }. Nếu có ứng dụng { -app-name } khác đang mở thư mục này, hãy thoát ứng dụng đó rồi thử lại. { $reason }
start-failed-network = { -app-name } không thiết lập được kết nối mạng theo các cài đặt mạng trong config.toml. { $reason }
start-failed = { -app-name } không khởi động được. Hãy thoát rồi mở lại. { $reason }
start-failed-remote = Không khởi động được chức năng điều khiển từ xa. { $reason }
start-failed-player = Không khởi động được trình phát. { $reason }
notice-episodes-queued = { $count ->
   *[other] Đã thêm { $count } tập vào hàng đợi tải xuống.
 }
