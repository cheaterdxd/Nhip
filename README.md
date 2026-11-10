# Nhịp

Ứng dụng quản lý công việc cá nhân bằng tiếng Việt. Hướng triển khai: backend Rust/SQLite trên Linux, desktop Svelte/Tauri cho Windows và Linux, PWA cho iPhone qua Tailscale.

Hiện là prototype kỹ thuật, dùng dữ liệu tổng hợp; chưa phải bản dùng thật.

- [Đặc tả kiến trúc](ARCHITECTURE.md)
- [Build, chạy và đo prototype](prototype/README.md)
- [Kết quả hiệu năng Linux](prototype/PERFORMANCE_REPORT.md)

## Trạng thái bàn giao

Đã đo lát cắt API/SQLite/UI trên Linux. Đang bổ sung thiết bị, lời nhắc có ID ổn định, xác nhận chung, Web Push, khay hệ thống và bộ đo đối chứng CPU. Mã các phần này là công việc đang triển khai; chưa hoàn tất kiểm chứng đầu cuối. Chưa thử trên Windows 11 và iPhone thật. Tailscale trên máy phát triển chưa chạy ở lần kiểm tra gần nhất.

Database, dữ liệu chạy thử, dependency và binary build được bỏ qua bởi Git. Chưa cấu hình khóa VAPID, tài khoản push hoặc thông tin xác thực triển khai.
