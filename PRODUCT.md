# Nhịp

<!-- impeccable:product-schema 1 -->

## Platform

web

Giao diện web thích ứng được dùng trong Tauri trên Windows/Linux và PWA trên iPhone. Backend riêng chạy trên Linux ở nhà.

## Stack

Người dùng đã yêu cầu prototype Svelte + Tauri và backend Rust để đo hiệu năng trước khi quyết định dùng cho ứng dụng thật. Kiến trúc đầy đủ và phạm vi đã chốt nằm tại ARCHITECTURE.md.

## Users

Một người dùng tiếng Việt, thường quên việc hoặc ghi rồi bỏ đó; cần quản lý dự án → task và nhắc việc mạnh.

## Product Purpose

Ghi nhận nhanh, biết việc tiếp theo, làm theo lịch có khoảng nghỉ và rà soát lý do hoãn/lỡ việc.

## Capabilities and Constraints

Prototype hiện tại đo danh sách task, ghi nhanh, lịch tuần, tìm giờ trống đơn phiên, Pomodoro và API/SQLite. Dữ liệu thử tổng hợp: 100 dự án, 10.000 lần thực hiện. Không coi prototype này là bản đầy đủ của mọi tính năng trong đặc tả.

## Brand Commitments

Người dùng yêu cầu phong cách iPhone clean, đơn giản và thực dụng. Nhịp là tên demo. Giữ nhận diện và luồng cơ bản từ demo v2, không thay thế bằng một thiết kế mới.

## Evidence on Hand

ARCHITECTURE.md chứa các quyết định sản phẩm; demo Nhịp v2 là tham chiếu UI. Chưa có kết quả benchmark trước khi chạy bộ đo; không đưa số liệu giả hoặc suy ra độ mượt trên Windows/iPhone từ Linux.

## Product Principles

- Phản hồi và trạng thái phải trung thực.
- Tốc độ chạy và tài nguyên phải được đo trên release build.
- AI không tự thay đổi dữ liệu chuẩn.
- Ghi nhanh ít bước, giao diện dễ đọc và thao tác rõ.
