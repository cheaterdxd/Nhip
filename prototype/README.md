# Nhịp — prototype đo Svelte + Tauri + Rust

Đây là lát cắt kiểm chứng hiệu năng, không phải bản sản phẩm đầy đủ của ARCHITECTURE.md. Database mới mặc định để trống; chỉ script benchmark mới chủ động nạp dữ liệu tổng hợp, không chứa công việc thật.

Prototype hiện có giao diện Svelte/Tauri, API Rust/Axum, SQLite, ghi nhanh, dự án/task, nhắc với giới hạn hoãn, lặp, lịch tuần/tháng, Pomodoro, báo cáo tuần và tìm giờ trống ở mức đang phát triển. AI chỉ là placeholder trên giao diện và chưa được client gọi. Ưu tiên hiện tại là vận hành app và sao lưu mã hóa thành file local. Chưa nghiệm thu thông báo trên thiết bị Windows/iPhone, đồng bộ ngoại tuyến đầy đủ, hoặc toàn bộ quy tắc nghiệp vụ của bản thật.

Phần thử nghiệm Codex CLI ở backend hiện để nguyên nhưng không được giao diện gọi; luồng AI chưa thuộc phạm vi vận hành hiện tại. Có thể nối lại sau khi các chức năng chính chạy ổn.

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

Khởi động lần đầu tạo database cá nhân trống tại `.data/nhip.sqlite` và mã truy cập riêng tại `.data/access-key` (quyền tệp 0600 trên Linux). Các database prototype cũ được giữ nguyên. Để nhập mã vào thiết bị khác, chạy `python3 scripts/run.py --show-key` trong terminal; mã chỉ hiện ở terminal đó.

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

Hoặc mở bản web tại http://127.0.0.1:4876 khi backend đang chạy. Mặc định backend chỉ bind loopback. Để app desktop trên máy khác kết nối trực tiếp qua Tailscale, đặt `NHIP_HOST` bằng IP Tailscale của PC Linux trong `.data/server.env`, khởi động lại backend, rồi nhập `http://<IP Tailscale>:4876` vào mục “Kết nối & thử nhắc việc” trên client. Lấy IP bằng `tailscale ip -4`. App desktop trên chính PC server cũng cần dùng địa chỉ này sau khi đổi bind. PWA cần HTTPS cho Service Worker và Web Push; Tailscale Serve có thể chuyển tiếp tới backend chạy ở loopback, hoặc dùng TLS proxy tương đương. Dữ liệu ứng dụng nằm ở `.data/nhip.sqlite`; dữ liệu tổng hợp chỉ được tạo trong các phép benchmark riêng.

### Chạy backend độc lập cùng systemd trên Linux

Backend có thể chạy riêng để các client khác vẫn kết nối khi đóng app desktop. Sau khi build release, dừng backend đang chạy từ `scripts/run.py` hoặc terminal, rồi tại thư mục `prototype` chạy:

```bash
python3 scripts/install_user_service.py
systemctl --user status nhip-backend.service
```

Unit chỉ chạy dưới tài khoản hiện tại, dùng thư mục dữ liệu `.data`, giữ mã truy cập trong tệp riêng mode 0600, tự khởi động lại khi backend lỗi và bật `UMask=0077`. Các biến tùy chọn cho Web Push có thể được đặt trong `.data/server.env` (mode 0600). Để unit tự chạy sau reboot trước khi người dùng đăng nhập, cần bật lingering một lần: `sudo loginctl enable-linger "$USER"`. Nếu không bật, unit sẽ chạy sau khi bạn đăng nhập Linux. Gỡ unit mà vẫn giữ database và mã truy cập bằng `python3 scripts/install_user_service.py --remove`.

Trước khi đăng ký Web Push cho thiết bị thật, tạo khóa VAPID một lần (dùng địa chỉ liên hệ của bạn):

```bash
python3 scripts/generate_vapid.py --subject mailto:you@example.com
```

Script lưu khóa riêng tại `.data/vapid.pem`, ghi cấu hình vào `.data/server.env` với quyền 0600 và từ chối ghi đè khóa/cấu hình có sẵn. Khởi động lại backend sau khi tạo khóa. Nếu cần khôi phục sau khi mất khóa VAPID, thiết bị phải đăng ký Web Push lại.

## Sao lưu mã hóa và khôi phục

**Phạm vi hiện tại: sao lưu file local thủ công.** Script dùng SQLite online backup để snapshot nhất quán trong lúc backend đang chạy, loại thông tin đăng ký thiết bị/Web Push, tạo manifest kèm SHA-256 và mã hóa trước khi xuất. Bản thủ công nhập cụm khôi phục ẩn, yêu cầu ít nhất 16 ký tự; file cuối không bị ghi đè. Tích hợp/tải tự động lên Google Drive và timer systemd đã được hoãn; các hướng dẫn/script phía dưới chỉ là phương án tương lai, chưa cần cấu hình hoặc chạy.

```bash
python3 scripts/backup.py export --database .data/nhip.sqlite --output /media/backup/nhip-2026-11-11.gpg
python3 scripts/backup.py verify --input /media/backup/nhip-2026-11-11.gpg
python3 scripts/backup.py restore --input /media/backup/nhip-2026-11-11.gpg --target .data/restore/nhip.sqlite
```

Giữ cụm khôi phục ở nơi khác PC; không tải lên Drive nếu không còn giữ được cụm đó. Có thể tải file `.gpg` đã mã hóa lên Drive thủ công, nhưng app hiện chưa tự đăng nhập Drive hoặc tải lên tự động. Khôi phục luôn tạo database mới; kiểm tra file đó trước, dừng backend trước khi tự quyết định thay database đang dùng. Sau khôi phục cần đăng ký lại thiết bị nhận thông báo. Dữ liệu giải mã tạm nằm trong thư mục tạm riêng của hệ thống và được xóa sau thao tác; để bảo vệ cả dữ liệu tạm khi máy bị truy cập vật lý, bật mã hóa ổ đĩa hệ điều hành.

Để chuẩn bị cho sao lưu không cần nhập mật khẩu mỗi ngày, tạo cặp khóa khôi phục trên máy đáng tin cậy, giữ khóa bí mật và cụm mật khẩu ngoài server, chỉ chép khóa công khai sang server. Lệnh tạo cặp khóa sẽ yêu cầu cụm mật khẩu:

```bash
gpg --quick-generate-key "Nhip backup recovery" rsa3072 encr 0
gpg --list-keys --keyid-format LONG
gpg --armor --export FINGERPRINT > nhip-recovery-public.asc
gpg --armor --export-secret-keys FINGERPRINT > nhip-recovery-private.asc
```

Thay `FINGERPRINT` bằng fingerprint đầy đủ. Sao chép khóa công khai vào server; giữ khóa bí mật và passphrase trên thiết bị lưu trữ riêng. `export --recipient-public-key` không hỏi mật khẩu và server không thể giải mã ciphertext; xác minh/khôi phục cần khóa bí mật:

```bash
python3 scripts/backup.py export --database .data/nhip.sqlite --output /media/backup/nhip-auto-2026-11-11.gpg --recipient-public-key /media/keys/nhip-recovery-public.asc
python3 scripts/backup.py verify --input /media/backup/nhip-auto-2026-11-11.gpg --identity /media/keys/nhip-recovery-private.asc
python3 scripts/backup.py restore --input /media/backup/nhip-auto-2026-11-11.gpg --target .data/restore/nhip.sqlite --identity /media/keys/nhip-recovery-private.asc
```

Ở chế độ khóa công khai, snapshot SQLite và manifest được kiểm tra trước khi mã hóa; server không thể giải mã ciphertext để tự làm round-trip. Hãy giữ khóa riêng và cụm mật khẩu để xác minh/khôi phục định kỳ. Để cấu hình Drive tự động, trước hết cài `rclone`, chạy `rclone config` để đăng nhập Google Drive dưới đúng tài khoản Linux, và đặt khóa công khai tại một đường dẫn server cố định. Sau đó cài timer:

```bash
python3 scripts/install_backup_timer.py --remote gdrive:nhip-backups --public-key .data/nhip-recovery-public.asc
systemctl --user list-timers nhip-drive-backup.timer
systemctl --user start nhip-drive-backup.service
```

Đổi `gdrive:nhip-backups` theo tên remote/thư mục Drive đã cấu hình. Xem [hướng dẫn cài rclone](https://rclone.org/install/) và [thiết lập Google Drive remote](https://rclone.org/drive/). Timer chạy lúc 02:00 giờ máy, có độ trễ ngẫu nhiên tối đa 5 phút. Sau khi upload, script so MD5 ciphertext trên Drive với file local; chỉ khi khớp mới áp dụng retention 7 ngày, 4 tuần và 3 tháng. Kết quả lần chạy gần nhất nằm ở `.data/backup-status.json`, lỗi chi tiết có trong `journalctl --user -u nhip-drive-backup.service`. OAuth token của rclone ở cấu hình riêng của rclone, không nằm trong snapshot SQLite. Gỡ timer bằng `python3 scripts/install_backup_timer.py --remove`.

## Chạy phép đo tái lập

Đóng backend dùng thử trước, vì bộ đo cần port 4876. Chạy khi các bước biên dịch đã hoàn tất, tránh đo trong lúc máy đang compile.

```bash
python3 scripts/benchmark.py --desktop-runs 20 --idle-seconds 60 --output results/latest.json
```

Kiểm tra riêng API lời nhắc, không đụng database đang dùng:

```bash
python3 scripts/check_reminders.py
node scripts/check_service_worker.mjs
```

Bài kiểm tra service worker mô phỏng sự kiện push và thao tác bấm; không thay thế việc nhận Web Push trên iPhone thật.

Kiểm tra luồng dữ liệu app, gồm ba lượt hoãn, cảnh báo cần quyết định, phiên tập trung, tái cấu trúc, tạo/tạm dừng/tiếp tục chuỗi lặp, báo cáo tuần, chống lệnh trùng và khởi động lại backend. Script tạo database tạm và tự xóa sau khi chạy:

```bash
python3 scripts/check_product.py
```

Quy tắc lặp sinh một lần kế tiếp, bù lại các lần ngày thường đã lỡ sau khi server ngừng, và chống bản trùng theo chuỗi + ngày địa phương. Kiểm thử thuật toán lặp (idempotency, ngày làm việc và ngày 31 ở tháng ngắn) bằng:

```bash
cargo test --manifest-path backend/Cargo.toml recurring::tests
```

## Thử PWA trên iPhone qua Tailscale

Kết quả ngày 11/11/2026: đã nhận một Web Push khi PWA đóng và chạm mở đúng task trên iPhone thật; server ghi gửi thành công ở lần đầu. Task thử đã hoàn thành. Focus, mất/kết nối lại VPN và chu kỳ nhắc lặp chưa được thử trên iPhone.

Backend cá nhân đang chạy bằng `nhip-backend.service` và Tailscale Serve chuyển tiếp HTTPS tới cổng local 4876. Trước khi thử, kiểm tra:

```bash
systemctl --user status nhip-backend.service
tailscale serve status
python3 scripts/run.py --show-key
```

Lệnh cuối chỉ in mã truy cập trong terminal trên PC; không gửi mã vào chat. Trên iPhone, kết nối cùng tailnet, mở `https://pc-home-linux.ancon-pauling.ts.net/` bằng Safari, chọn **Thêm vào Màn hình chính** và mở app từ biểu tượng vừa cài. Trong mục **Kết nối & thử nhắc việc**, nhập mã truy cập rồi bấm **Lưu kết nối**. Sau khi app tải lại và hiển thị dữ liệu, bấm **Đăng ký thông báo Web Push** và chọn **Cho phép** trong hộp thoại iOS. Cài PWA và lưu mã chưa tự đăng ký kênh nhận push.

Để kiểm tra một luồng, tạo task thử riêng trong app, bấm **Nhắc thử sau 10 giây**, đóng app, chờ thông báo rồi chạm để mở đúng task. Kiểm tra thêm thao tác hoãn với lý do, nhận nhắc lại và trạng thái cập nhật trên thiết bị khác. Sau đó thử mất rồi kết nối lại Tailscale; thử riêng khi bật Focus. Chỉ kết quả trên iPhone thật mới được coi là nghiệm thu Web Push.

Script `iphone_preview.py` vẫn có thể tạo môi trường thử độc lập khi Serve chưa được dùng cho backend cá nhân. Nó từ chối thay cấu hình Serve đang có, nên không dùng script này với cấu hình hiện tại.

## Chuẩn bị thử Windows 11

Có workflow GitHub Actions chạy thủ công tại `.github/workflows/windows-prototype.yml`: nó build bộ cài NSIS trên Windows và lưu artifact `nhip-windows-prototype` trong bảy ngày. Workflow chỉ có hiệu lực sau khi mã chứa nó được đưa lên GitHub, và bản build phải thành công trước khi có artifact. Bộ cài chưa được ký mã; hành vi cài đặt và thông báo vẫn cần kiểm tra trên máy Windows thật.

Nếu tự build trên Windows, cài Node.js, Rust MSVC, Microsoft C++ Build Tools với workload Desktop development with C++, và WebView2 theo [hướng dẫn Tauri](https://v2.tauri.app/start/prerequisites/). Trong PowerShell, tại thư mục `prototype`:

```powershell
npm ci
npm run check
npx tauri build --bundles nsis
```

Chạy file `*-setup.exe` được tạo trong `src-tauri\target\release\bundle\nsis`, mở app và nhập URL HTTPS của server và mã thử trong mục **Kết nối & thử nhắc việc**. Kết nối Windows và server vào cùng tailnet trước; nếu chưa chạy backend, giao diện vẫn có khu vực cấu hình nhưng dữ liệu ban đầu sẽ báo lỗi kết nối. Sau khi lưu và tải lại, đăng ký kênh nhắc trên máy Windows, tạo lời nhắc thử, đóng cửa sổ vào khay, rồi kiểm tra nhận thông báo, bấm mở task và xác nhận. Đo CPU/RAM và thử lại sau khi Windows khởi động lại. Một bộ cài build thành công không thay thế các bước thử này.

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
