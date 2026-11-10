# Nhịp — prototype đo Svelte + Tauri + Rust

Đây là lát cắt kiểm chứng hiệu năng, không phải bản sản phẩm đầy đủ của ARCHITECTURE.md. Dữ liệu là tổng hợp, không chứa công việc thật.

Có: giao diện Svelte, bộ host Tauri, API Axum/Tokio, SQLite thật với 100 dự án/10.000 lần thực hiện, ghi nhanh/hoàn thành task, lịch tuần, đề xuất một phiên tối đa 50 phút, Pomodoro và lưu phiên. Chưa có: nhắc đa thiết bị, sao lưu Drive, AI, đăng nhập, đồng bộ và toàn bộ quy tắc nghiệp vụ của bản thật.

## Build release

Cần Node, Rust, C compiler và thư viện Linux theo tài liệu Tauri. Trên Ubuntu dùng libwebkit2gtk-4.1-dev, libgtk-3-dev, libayatana-appindicator3-dev, librsvg2-dev, libssl-dev, libxdo-dev, pkg-config, build-essential, patchelf. Rust phải ở PATH (hoặc dùng đường dẫn đầy đủ tới cargo).

```bash
cd prototype
npm ci
npm run check
npm run build
cargo build --release --manifest-path backend/Cargo.toml
cargo build --release --features custom-protocol --manifest-path src-tauri/Cargo.toml -j 6
cargo test --release --manifest-path backend/Cargo.toml
```

**Quan trọng:** khi gọi Cargo trực tiếp cho Tauri release, phải có `--features custom-protocol`. Nếu không, Tauri có thể vẫn trỏ tới devUrl Vite 1420 và hiện Connection refused khi server Vite không chạy. `npm run desktop:build` dùng Tauri CLI để cấu hình production build; lệnh Cargo ở trên dùng cho bộ đo trực tiếp binary, chưa tạo bộ cài.

Cách mở đơn giản (script tự bật backend trước và chỉ dừng backend do chính nó tạo khi bạn đóng app):

```bash
python3 scripts/run.py
```

Hoặc mở bằng hai terminal, cùng thư mục `prototype`:

```bash
backend/target/release/nhip-backend
```

```bash
src-tauri/target/release/nhip-desktop
```

Hoặc mở bản web tại http://127.0.0.1:4876 khi backend đang chạy. Backend chỉ bind loopback; không đưa prototype chưa có xác thực này lên địa chỉ công khai. Dữ liệu dùng thử ở `.data/prototype.sqlite`.

## Chạy phép đo tái lập

Đóng backend dùng thử trước, vì bộ đo cần port 4876. Chạy khi các bước biên dịch đã hoàn tất, tránh đo trong lúc máy đang compile.

```bash
python3 scripts/benchmark.py --desktop-runs 20 --idle-seconds 60 --output results/latest.json
```

Script tự tạo database thử mới trong `.bench-work`, chạy backend release, đo HTTP đọc/ghi SQLite, bốn client đồng thời, mở/đóng Tauri 20 lần, đo render trong WebKit và CPU/RAM lúc nghỉ 60 giây. Nó dừng các process do chính nó tạo khi xong; không dừng process lạ chiếm port.

Mỗi lần đo có raw JSON, log và database riêng. `results/latest.json` giữ đường dẫn work_dir; giữ work_dir nếu cần đối chiếu. Có thể chạy riêng API bằng `--skip-desktop`; kết quả đó không chứng minh hiệu năng Tauri.

## Cách đọc đúng số liệu

- API: thời gian round trip qua HTTP loopback, bao gồm SQLite và đọc response; không phải Tailscale/WAN.
- Ghi: mỗi request tạo/hoàn thành có commit thật, WAL + synchronous FULL. Seed dữ liệu là transaction riêng trước đo.
- Startup: một process mới mỗi lần, nhưng filesystem/OS/WebView cache có thể đã ấm. Không gọi đây là đo cold boot. Thời gian ngoài process từ spawn tới nhận file kết quả có sai số lấy mẫu tối đa khoảng 10 ms.
- UI: từ thay đổi state tới hai requestAnimationFrame; là proxy tổng hợp cho render, không phải INP hoặc độ trễ chạm của người dùng thật. Số mẫu chuyển màn hình 50, chọn task 30. Dữ liệu lấy trực tiếp trong WebView Tauri, không thay bằng kết quả Chrome.
- RAM: Linux PSS của toàn cây process Tauri/WebKit khi đọc được; có cả danh sách process và trạng thái thiếu quyền. Không lấy riêng Rust host để quảng cáo app nhẹ.
- CPU nghỉ: phần trăm một core theo `/proc` trong cửa sổ 60 giây, không bật Pomodoro. Không đo cùng tiến trình compiler.
- Dataset tải dày cố ý chứa những lịch quá tải. Đo cả tìm lịch không khả thi và tuần tương lai còn trống; bộ đề xuất prototype chưa xử lý chia task dài thành nhiều phiên.
- Kết quả Linux không suy ra kết quả Windows, iPhone, push notification, Tailscale hoặc đầy đủ sản phẩm. Số đo một stack cũng không chứng minh nó nhanh nhất trong tất cả lựa chọn.

## Kết quả đã đo

[PERFORMANCE_REPORT.md](PERFORMANCE_REPORT.md) chứa kết quả Linux release, phần đạt/chưa đạt và giới hạn môi trường. [results/linux-release.json](results/linux-release.json) là dữ liệu gốc của lượt đo hoàn tất; [results/source-sha256.json](results/source-sha256.json) lưu hash mã nguồn tại thời điểm lập báo cáo. API và proxy phản hồi UI đạt ngưỡng; CPU desktop lúc nghỉ chưa đạt, Windows/iPhone và cold startup còn phải đo.
