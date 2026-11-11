# Kết quả prototype Svelte + Tauri + Rust trên Linux

## Kiểm chứng iPhone sau lượt đo Linux

Ngày 11/11/2026, backend cá nhân chạy bằng systemd qua Tailscale Serve HTTPS. Một task thử riêng được đặt nhắc sau 60 giây; `app_push_delivery` ghi `sent` ở lần gửi đầu. Người dùng xác nhận iPhone nhận thông báo khi PWA đã đóng và chạm thông báo mở đúng task. Task thử đã được đánh dấu hoàn thành để dừng nhắc lặp. Kết quả này chứng minh một luồng Web Push thực tế; chưa kiểm chứng Focus, mất rồi kết nối lại Tailscale, nhiều lần nhắc liên tiếp hoặc Windows.

Lượt đo hoàn tất ngày 10/11/2026 theo giờ máy. Dữ liệu gốc: [results/linux-release.json](results/linux-release.json). Cách build, mở app và chạy lại: [README.md](README.md).

**Kết luận:** tốc độ API, tìm lịch và phản hồi giao diện đạt mục tiêu trong lát cắt đã đo. Khởi động khi cache đã ấm dưới 2 giây. Chưa thông qua toàn bộ cổng hiệu năng: CPU desktop lúc nghỉ còn cao, startup nguội và các nền tảng khác chưa đo. Có cơ sở tiếp tục với ứng viên này, chưa có cơ sở khẳng định đây là stack nhanh nhất.

## Môi trường và phương pháp

- Ubuntu 26.04.1, x86_64, kernel 7.0.0-43; Xeon E5-2680 v4 2,4 GHz, 56 CPU logic; RAM khoảng 32 GB, lưu trữ NVMe. Máy làm việc dùng chung, không cô lập CPU; chưa xác minh đường tăng tốc GPU hoặc chế độ nguồn điện.
- Release build: Svelte 5.57.3, Vite 7.3.8, Tauri 2.12.5, Rust 1.99.1; GTK 3.24.52, WebKitGTK 2.52.6. Backend Axum/Tokio, SQLite WAL, synchronous FULL.
- Database mới gồm 100 dự án và 10.000 lần thực hiện: 9.500 lịch sử đã xong, 500 còn hoạt động. Lượt đo thêm 600 bản ghi thật, kết thúc ở 10.600. Không render 10.000 dòng cùng lúc: lượt UI có 18 task hôm nay và 126 sự kiện tuần.
- 4.000 request HTTP được tính thời gian, không tính warmup. API chạy qua loopback, chưa qua Tailscale. Ghi task có commit SQLite thật.
- 20 lần mở process Tauri mới; cache hệ điều hành/filesystem/WebView có thể đã ấm. Thời gian từ đầu hàm main Rust tới khi nhận kết quả từ UI sau tải API và hai khung hình; có thêm phép đo bên ngoài từ spawn tới file kết quả trong JSON.
- UI: 50 lần đổi màn hình, 30 lần chọn task, từ thay đổi state tới Svelte tick và hai requestAnimationFrame trong WebView Tauri. Đây là phép đo tổng hợp, chưa phải INP hay độ trễ thao tác thật.
- CPU/RAM: cửa sổ mở đứng yên 60,09 giây, Pomodoro chưa chạy. RAM là PSS toàn cây Tauri gồm Rust host, WebKitNetworkProcess và WebKitWebProcess; tất cả process đọc được. CPU tính theo phần trăm một nhân, không chia cho 56 CPU logic.

## Số đo

p95 là mức mà khoảng 95% mẫu không vượt quá. Các số dưới đây được làm tròn từ JSON.

| Phép đo | Số mẫu | Trung vị | p95 | Đánh giá theo mục tiêu |
|---|---:|---:|---:|---|
| Mở Tauri, cache ấm | 20 | 1.652 ms | 1.684 ms | Dưới 2 giây; chưa thay được bài mở nguội |
| Chuyển Hôm nay/lịch tuần | 50 | 32 ms | 48 ms | Đạt mục tiêu proxy UI dưới 100 ms |
| Chọn task đã tải | 30 | 32 ms | 33 ms | Đạt mục tiêu proxy UI dưới 100 ms |
| Đọc task hôm nay | 1.000 | 1,34 ms | 1,40 ms | Đạt dưới 50 ms |
| Đọc lịch tuần | 500 | 1,77 ms | 1,97 ms | Đạt dưới 50 ms |
| Tìm lịch trong tuần dày | 500 | 1,06 ms | 1,24 ms | Đạt dưới 200 ms; trả không có chỗ phù hợp |
| Tìm lịch trong tuần còn trống | 500 | 0,77 ms | 0,93 ms | Đạt dưới 200 ms; có đề xuất hợp lệ |
| Tổng hợp số liệu báo cáo | 100 | 5,09 ms | 5,42 ms | Chưa gồm phân tích AI |
| Tạo task và commit | 500 | 2,37 ms | 2,51 ms | Đạt dưới 50 ms |
| Hoàn thành task và commit | 500 | 2,33 ms | 2,45 ms | Đạt dưới 50 ms |
| Hỗn hợp bốn client đồng thời | 400 | 4,72 ms | 6,86 ms | Đạt dưới 50 ms trong tải thử |

Lần chuyển màn hình chậm nhất là **147 ms**, dù p95 đạt 48 ms. Không diễn giải kết quả thành mọi thao tác đều dưới 100 ms. Đây chỉ là một lượt đo hoàn chỉnh trên một máy.

| Tài nguyên lúc nghỉ | Backend | Toàn cây desktop |
|---|---:|---:|
| PSS trung vị | 6,94 MiB | 110,33 MiB |
| PSS lớn nhất trong cửa sổ đo | 6,94 MiB | 118,40 MiB |
| CPU theo một nhân | 0,00% | 10,45% |

Backend không tăng CPU tick trong cửa sổ lấy mẫu; 0,00% không có nghĩa không bao giờ dùng CPU. Desktop **chưa đạt yêu cầu CPU nghỉ gần không**. Theo số tick, phần lớn nằm ở Rust/GTK host, phần còn lại ở WebKit WebProcess. Kiểm tra mã UI không thấy animation hoặc polling liên tục khi Pomodoro tắt; điều đó chưa xác định nguyên nhân. Cần profile event loop/render và đối chiếu cửa sổ Tauri tối thiểu, trạng thái focus/ẩn/khay, đường GPU trên cùng máy trước khi đổi framework. Chưa đo CPU khi hoạt động hay sau thu nhỏ vào khay.

## Kiểm chứng chức năng và lỗi khởi động

- Frontend kiểm tra kiểu: 0 lỗi, 0 cảnh báo; build production thành công. JavaScript chính khoảng 69,51 kB, gzip 26,42 kB; CSS 7,61 kB, gzip 2,15 kB. Đây là kích thước asset, không phải RAM hay kích thước bộ cài.
- Ba test Rust đạt: dữ liệu và ràng buộc, slot hợp lệ và cuối tuần công ty, từ chối hoàn thành hai lần bằng phiên bản cũ. Bộ đo HTTP xác minh request phiên bản cũ bị từ chối bằng 409.
- Ghi nhanh task/note trên giao diện đã lưu vào SQLite và còn sau tải lại. Pomodoro có đường lưu phiên vào backend; chưa kiểm chứng đầy đủ khôi phục sau crash.
- Đã sửa lỗi ảnh “Could not connect to 127.0.0.1: Connection refused”: Tauri được build Cargo trực tiếp mà thiếu `custom-protocol`, nên vẫn trỏ devUrl Vite. Binary release đã build lại với feature đó, tải giao diện đóng gói và qua đủ 20 lần khởi động. Script `scripts/run.py` bật backend trước khi mở desktop để tránh lỗi API do backend chưa chạy.

## Phạm vi chưa được chứng minh

Chưa đo Windows/WebView2, Safari iPhone/PWA, Tailscale và mất kết nối, startup nguội sau khởi động máy, nhận nhắc nền/đa thiết bị, khay hệ thống, IME tiếng Việt đầy đủ, accessibility, AI, khôi phục backup Google Drive, đồng bộ dồn và báo cáo đồng thời với nhắc việc. Prototype chưa có xác thực, task lặp đầy đủ, chia task dài thành nhiều phiên hay toàn bộ chính sách hoãn. Không dùng nó như bản sản phẩm thật.

Trước khi cố định stack cho bản chính: xử lý hoặc khoanh vùng CPU nghỉ; đo cold startup, Windows và iPhone trên máy đích; kiểm chứng các khả năng nền bắt buộc. Chỉ dựng đối chứng framework khác nếu profile cho thấy nút thắt ở lớp UI/WebView hoặc cần chứng minh lựa chọn nhanh hơn ứng viên cụ thể.
