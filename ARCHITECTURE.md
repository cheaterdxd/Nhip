# Nhịp — đặc tả sản phẩm và kiến trúc ứng dụng quản lý công việc cá nhân

Phiên bản đặc tả: 2.2 · Ngày rà soát: 10/11/2026.

Trạng thái: giai đoạn thiết kế, chưa xây ứng dụng thật. Demo chỉ để góp ý giao diện, không phải danh sách đầy đủ tính năng hay nguồn dữ liệu vận hành.

## 1. Mục tiêu, người dùng và trạng thái quyết định

Ứng dụng dành cho một người thường quên việc hoặc ghi kế hoạch rồi bỏ đó. Mục tiêu là giúp ghi nhận ngay, biết bước tiếp theo, được nhắc lại có quy tắc và nhìn được nguyên nhân kế hoạch không thành để điều chỉnh. Công việc tổ chức theo **dự án → task**.

### 1.1. Đã được người dùng chốt

- Backend chạy trên PC Linux ở nhà, bật cả ngày; dữ liệu chuẩn nằm ở backend.
- Có ứng dụng desktop cài trên **cả Windows và Linux**, nhập địa chỉ backend rồi sử dụng. Cùng PC Linux có thể chạy cả backend và client.
- iPhone dùng web app cài vào màn hình chính; kết nối dữ liệu riêng qua Tailscale. Chưa cần ứng dụng iOS phát hành qua App Store.
- Một người dùng, một mã khóa riêng; nhận lời nhắc trên toàn bộ thiết bị đã đăng ký.
- Không đặt giờ im lặng trong app. Khi không phản hồi, tiếp tục nhắc mỗi 3 giờ sau đợt nhắc tăng dần ban đầu.
- Mỗi lần thực hiện được hoãn tối đa ba lần; hết lượt phải làm, tái cấu trúc hoặc bỏ. Chỉ giới hạn thao tác task liên quan, không chặn toàn bộ app.
- Lịch riêng có tháng/tuần; lịch ngày và chương trình trong ngày hỗ trợ thao tác trên điện thoại.
- Khung có thể đề xuất là 07:00–24:00. Công ty: thứ Hai–Sáu, 08:30–12:00 và 13:30–18:00. Ngoài các khung công ty là thời gian cá nhân; cuối tuần chỉ xếp việc cá nhân.
- Sức chứa khởi đầu: ngày thường 5 giờ task công ty + 2 giờ task cá nhân; cuối tuần 4 giờ cá nhân. Có thể chỉnh và giảm riêng một ngày.
- Task nhập thời lượng, có thể chia nhiều phiên; có Pomodoro ghi thời gian thực tế.
- Task lặp ngày/tuần/tháng: lần lỡ vẫn quá hạn, lần kế tiếp vẫn được tạo; hẹn lại lần lỡ bắt buộc lý do.
- Báo cáo tối Chủ nhật gồm kết quả, lý do hoãn/lỡ việc và đề xuất cải thiện; có lịch sử để xem lại.
- AI phân tích task/note và báo cáo qua agent CLI; chỉ nhận nội dung được cho phép gửi. Kết quả AI và đề xuất lịch đều phải được duyệt trước khi áp dụng.
- Sao lưu dữ liệu vận hành lên Google Drive.
- Giao diện tiếng Việt, phong cách ứng dụng iPhone: sạch, đơn giản, thực dụng, chữ rõ và thao tác dễ hiểu. Desktop có bố cục phù hợp màn hình lớn.
- Các thiếu sót thấy trong demo như tạo dự án, báo cáo và mức năng lượng phải có trong bản thật; không cần tiếp tục mở rộng demo để thay cho triển khai.

### 1.2. Mặc định thiết kế, có thể điều chỉnh

Nhịp nhắc ban đầu, thời gian Pomodoro/nghỉ, giờ tạo báo cáo, lịch sao lưu, quy tắc ngày cuối tháng và các giá trị vận hành cụ thể bên dưới là mặc định đề xuất. Chúng không làm thay đổi các nguyên tắc cứng đã chốt.

Các công nghệ tại mục 10.2 vẫn là đề xuất, chưa được người dùng xác nhận. Các việc cần chốt/kiểm chứng trước triển khai nằm tại mục 15.

## 2. Phạm vi bản đầu và cách chia mốc

Bản đầu hoàn chỉnh gồm: tạo/quản lý dự án và task; Hộp thư đến; ghi nhanh task/note; nhắc đa thiết bị và quy tắc ba lần hoãn; task lặp; lịch riêng và tìm giờ trống; năng lượng/mức tập trung; Pomodoro; báo cáo tuần; phân tích AI chờ duyệt; truy cập riêng và sao lưu/khôi phục.

Chia thành các mốc để kiểm chứng, không coi các tính năng chưa làm ở mốc đầu là đã bị loại khỏi bản đầu:

1. **Mốc nền tảng:** dữ liệu, đăng nhập, dự án/task/ghi nhanh, đồng bộ; thử thông báo trên cả ba loại client.
2. **Mốc công việc hằng ngày:** nhắc/hoãn, task lặp, lịch, năng lượng và Pomodoro.
3. **Mốc hoàn thiện:** báo cáo số liệu, một adapter AI, sao lưu/khôi phục, hoàn thiện giao diện và bộ cài Windows/Linux.

Chưa nằm trong bản đầu: nhiều người dùng/cộng tác, tích hợp lịch ngoài, chỉnh sửa toàn bộ dữ liệu ngoại tuyến, ứng dụng iOS native và tự động cho AI thay đổi dữ liệu không cần duyệt. Các tệp đính kèm chưa phải tính năng bắt buộc; thiết kế sao lưu cần mở rộng được khi bổ sung chúng.

## 3. Dự án, task, ghi nhanh và note

### 3.1. Dự án

- Có nút **Dự án mới** trên desktop và iPhone; nhập tên, chọn Công ty/Cá nhân, mô tả tùy chọn.
- Trạng thái: Đang hoạt động / Tạm dừng / Hoàn thành / Lưu trữ. Đổi trạng thái không âm thầm hoàn thành hoặc xóa task con; nếu còn task mở, phải hiển thị phương án xử lý chúng.
- Mở dự án thấy task đúng dự án, bước tiếp theo, lịch và thời gian đã làm; có thể thêm task ngay trong dự án.
- Dự án đang hoạt động mà không có hành động tiếp theo xuất hiện trong phiên rà soát.
- Task được thừa hưởng không gian từ dự án. Đổi không gian của dự án phải xem trước tác động tới các khối lịch và quyền gửi AI; không tự dời lịch.

### 3.2. Task và lần thực hiện

Task có tên, dự án, note, ưu tiên, thời điểm có thể bắt đầu, hạn chót tùy chọn, thời lượng dự kiến ban đầu, ước lượng còn lại và mức tập trung cần thiết. Có thể tạo/sửa, hoàn thành, bỏ hoặc tái cấu trúc; giữ lịch sử thay đổi.

**Hạn chót**, **khối thời gian làm việc**, **lời nhắc**, **thời lượng dự kiến**, **thời gian thực tế** và **ước lượng còn lại** là các thông tin riêng.

- Task thường có một lần thực hiện; task lặp có nhiều lần thực hiện độc lập. Mọi quy tắc trạng thái/hoãn áp dụng trên lần thực hiện.
- Trạng thái lần thực hiện: Chưa làm rõ / Sẵn sàng / Đã xếp lịch / Đang làm / Đang chờ / Cần quyết định / Hoàn thành / Đã bỏ / Đã tái cấu trúc. Quá hạn là dấu hiệu tính từ lịch/hạn, không thay thế trạng thái nghiệp vụ.
- Đang chờ phải ghi lý do và thời điểm kiểm tra lại; không tự đặt lại lượt hoãn. Bản đầu hỗ trợ trạng thái chờ, chưa cần đồ thị phụ thuộc task phức tạp.
- Khi bỏ hoặc tái cấu trúc, lưu lý do và liên kết gốc–mới. Khi hoàn thành, ngừng các lời nhắc và giải phóng khối làm việc chưa diễn ra; vẫn giữ lịch sử kế hoạch và thời gian đã làm.
- Có Hoàn tác cho hoàn thành/bỏ nhầm. Backend kiểm tra phiên bản hiện tại; không ghi đè mù nếu đã có thay đổi mới.

### 3.3. Ghi nhanh và note

- Một ô nhập chính, không bắt buộc chọn dự án, thời lượng, hạn chót hoặc năng lượng khi ghi nhanh. Nội dung chưa phân loại vào **Hộp thư đến**.
- Lưu bản nháp cục bộ trước; đóng form/mất mạng không làm mất nội dung. Hiện rõ Chưa đồng bộ; gửi lại có mã chống tạo trùng.
- Hộp thư đến có phiên làm rõ: chuyển thành task/note, chọn dự án và các thông tin cần thiết. Chưa có thời lượng thì chưa xếp lịch tự động được; app phải chỉ rõ bước cần bổ sung.
- Note giữ nguyên văn bản gốc. Khi mở phải thấy đúng nội dung vừa lưu; hỗ trợ sửa, liên kết tới dự án/task và tạo task từ note.
- AI có thể đề xuất tóm tắt, hành động, dự án liên quan hoặc câu hỏi còn thiếu. Người dùng xem và chọn phần được lưu; không ghi đè bản gốc.

## 4. Nhắc việc, hoãn và yêu cầu quyết định

### 4.1. Lượt nhắc và phản hồi

- Backend tạo lượt nhắc có ID, lịch và phiên bản trạng thái. Ghi riêng kết quả gửi theo từng thiết bị; không gọi việc gửi thành công tới dịch vụ push là người dùng đã đọc.
- Mọi thiết bị đã đăng ký nhận lượt nhắc. Đóng thông báo hệ điều hành không được tính là phản hồi và không tăng bộ đếm hoãn.
- Phản hồi gồm Hoàn thành / Bắt đầu / Hoãn / Tái cấu trúc / Bỏ. Xử lý trên một máy cập nhật trạng thái chung; các máy khác đối chiếu để ngừng chuỗi nhắc cũ.
- Mặc định: nhắc tại thời điểm đã đặt, nhắc lại sau 15 phút, sau 1 giờ, sau 3 giờ tính từ lần nhắc trước; tiếp theo mỗi 3 giờ cho tới khi có phản hồi. Không có giờ im lặng trong app. Hệ điều hành vẫn quyết định việc hiển thị theo quyền/Focus của thiết bị.
- Theo dõi riêng lượt chưa phản hồi, lần hoãn và lỗi gửi; không quy lỗi kỹ thuật thành người dùng lơ việc.

### 4.2. Giới hạn ba lần hoãn

1. Mỗi lần chủ động dời một cam kết đã đến hạn tăng bộ đếm một lần. Nút Hoãn, kéo lịch, sửa ngày/giờ nhắc đều đi qua cùng quy tắc backend.
2. Xếp lịch lần đầu hoặc sửa lịch tương lai trước khi cam kết đến hạn không tính là hoãn, nhưng vẫn lưu lịch sử. Sau khi đã đến hạn, đổi trạng thái Chờ hoặc nhập lý do khách quan không miễn lượt hoãn.
3. Cho phép hoãn tối đa ba lần. **Lịch hẹn của lần thứ ba vẫn có hiệu lực**; khi đến lịch đó mà chưa hoàn thành, chuyển sang Cần quyết định. Không cho hoãn lần thứ tư.
4. Task Cần quyết định chỉ có Làm ngay / Tái cấu trúc / Bỏ. Dừng chuỗi thông báo nhắc thông thường của task đó, giữ thẻ yêu cầu quyết định nổi bật. Không khóa phần còn lại của ứng dụng.
5. Bắt đầu phải mở một phiên làm việc có điểm kiểm tra lại. Nếu phiên hết/gián đoạn mà việc chưa xong và đã hết lượt hoãn, trở lại Cần quyết định; không thể bấm Bắt đầu để ẩn task vô thời hạn.
6. Tái cấu trúc yêu cầu lý do, hành động cụ thể mới và thời lượng mới; chỉ sửa tên hoặc dời ngày không đủ. Task gốc đóng ở trạng thái Đã tái cấu trúc, các task mới liên kết về gốc. Task mới có bộ đếm riêng nhưng báo cáo giữ cả chuỗi hoãn/tái cấu trúc, không xóa lịch sử để làm số liệu đẹp hơn.
7. Mọi lần hoãn có lý do để phục vụ rà soát; hẹn lại lần lặp đã lỡ bắt buộc lý do. Có lý do chọn nhanh và phần ghi thêm, tránh form dài.

### 4.3. Giảm trùng và giới hạn nền tảng

- Các việc tới hạn trong cùng một phút được gom trên từng thiết bị. Dùng ID ổn định để cập nhật thông báo thay vì nhân bản; không làm mất mục chưa xử lý khi gom.
- Desktop có tối đa một cửa sổ nhắc nổi của app cùng lúc; các mục khác nằm trong danh sách. Không tự giành bàn phím khi người dùng đang nhập. Pomodoro vẫn nhận thông báo nhưng không bị tự chuyển task.
- Desktop chạy nền ở khay hệ thống, có tự khởi động khi đăng nhập, lưu lịch nhắc sắp tới. Máy ngủ/tắt không thể bảo đảm hiển thị đúng giờ; khi thức dậy đối chiếu và tổng hợp việc đã lỡ, không phát lại toàn bộ chuỗi cũ.
- iPhone PWA nhận Web Push từ backend. Không hứa có bộ lập lịch cục bộ tương đương desktop khi backend mất mạng. Khi mở dữ liệu app cần Tailscale kết nối; thông báo push có thể tới trước khi VPN kết nối lại.
- Không bảo đảm gỡ ngay thông báo iPhone đã hiển thị khi xử lý task trên máy khác. Chạm thông báo luôn mở trạng thái mới nhất sau đối chiếu; thông báo lỗi thời không được mở lại task đã xong.
- Có nút **Thử thông báo** và màn hình quyền/trạng thái từng thiết bị. Khi quyền bị tắt hoặc đăng ký push hết hiệu lực, hướng dẫn xử lý rõ.

## 5. Task lặp và tồn đọng

- Hỗ trợ lặp theo lịch ngày, các ngày trong tuần, hoặc ngày trong tháng. Mỗi lần được sinh có ID độc lập và khóa duy nhất theo quy tắc + kỳ; chạy lại sau khởi động không tạo trùng.
- Lần lỡ vẫn quá hạn trong khi lần tiếp theo được tạo. Hẹn lại/bỏ một lần không tự thay đổi quy tắc cho tương lai.
- Hiển thị tồn đọng theo nhóm, ví dụ “7 lần chưa xử lý”; mở được từng lần. Có xử lý hàng loạt với xem trước, lý do và kiểm tra giới hạn từng lần. Không áp một lệnh Hoãn hàng loạt cho lần đã hết lượt.
- Sửa **lần này** hoặc **từ lần này trở đi**; giữ nguyên lịch sử và lý do đã ghi. Có tạm dừng, tiếp tục, kết thúc quy tắc mà không xóa các lần cũ.
- Mặc định theo múi giờ Asia/Ho_Chi_Minh; lịch thứ Hai là đầu tuần. Lặp ngày 29/30/31 mà tháng không có ngày đó thì dùng ngày cuối tháng. Đổi múi giờ/giờ lặp phải xem trước ảnh hưởng, không sinh trùng lần.
- Với task công ty có ngày dự kiến rơi cuối tuần, giữ kỳ gốc và đề xuất giờ làm vào ngày công ty kế tiếp; không tự đổi hạn chót hoặc tạo thêm một lần ngoài quy tắc. Ngày lễ/ngày nghỉ riêng có thể được đánh dấu Bận/Không xếp lịch.

## 6. Lịch, sức chứa, mức tập trung và năng lượng

### 6.1. Lịch riêng và đầu vào

- Có lịch tháng/tuần/ngày; phân biệt sự kiện cố định, khối Bận/nghỉ/di chuyển, phiên làm việc đã cam kết và khối đề xuất chưa duyệt.
- Tạo/sửa khối Bận nhanh; sự kiện chỉ chiếm thời gian thực tế trên lịch. Không đánh đồng số giờ công ty mở với số giờ task có thể làm.
- Giữ đúng khung công ty/cá nhân ở mục 1. Mặc định nghỉ 10 phút giữa các khối task; khoảng nghỉ Pomodoro đã có được tính vào khoảng đệm để tránh cộng hai lần.
- Sức chứa 5 giờ công ty + 2 giờ cá nhân ngày thường, 4 giờ cá nhân cuối tuần là **trần task để đề xuất**, không phải mục tiêu cần lấp. Sự kiện, khoảng nghỉ và việc bận vẫn phải vừa trong lịch. Trước khi duyệt, hiện tải theo từng không gian và tổng lịch ngày.
- Người dùng có thể đặt thêm thủ công khi vượt trần task; app cảnh báo và yêu cầu xác nhận việc vượt trần, không âm thầm nới giới hạn cho bộ đề xuất.

### 6.2. Mức tập trung của task và năng lượng hiện tại

- Form tạo đầy đủ/chi tiết task có **Mức tập trung cần thiết**: Nhẹ / Vừa / Cao / Chưa xác định. Đây là yêu cầu công việc, không phải đánh giá năng lực người dùng.
- Hôm nay có **Năng lượng hiện tại**: Thấp / Bình thường / Cao, có thể bỏ qua hoặc đổi trong ngày. Lưu từng lần chọn với thời điểm; không tự suy ra sức khỏe.
- Năng lượng thấp ưu tiên việc nhẹ hoặc phiên ngắn; năng lượng cao có thể ưu tiên việc cần tập trung sâu. Không loại bỏ việc khẩn chỉ vì mức tập trung cao: nêu rõ xung đột và cho chọn.
- Chưa có năng lượng hoặc chưa biết mức tập trung thì vẫn đề xuất bằng lịch/thời lượng. Lựa chọn hiện tại chỉ dùng cho phần còn lại của ngày; không giả định bạn giữ mức năng lượng đó suốt tuần.
- Đổi năng lượng có thể đề xuất giảm sức chứa ngày; không tự thay trần, dời cam kết hoặc đặt lại lượt hoãn. Mọi thay đổi lịch vẫn phải duyệt.

### 6.3. Bộ tìm giờ trống

- Đầu vào: task/lần thực hiện, thời lượng còn lại, ngày có thể bắt đầu, hạn chót, ưu tiên, không gian, trạng thái chờ, mức tập trung, lịch Bận, cam kết, khoảng nghỉ và sức chứa.
- Đầu ra: các khung giờ khả thi kèm lý do, hoặc **Không xếp vừa** kèm ràng buộc gây thiếu thời gian. Không ép tạo deadline giả cho việc chưa có hạn.
- Không xếp task Đang chờ, không xếp trước ngày có thể bắt đầu, không lấn giờ cá nhân cho việc công ty. Không tự sửa hạn chót.
- Task dài được đề xuất nhiều phiên, mặc định mỗi phiên tối đa 50 phút; phần cuối có thể ngắn hơn. Tổng phiên phải khớp thời lượng còn lại, khoảng nghỉ không tính là thời gian làm.
- Khi nhiều việc cạnh tranh: ưu tiên hạn cứng có nguy cơ trễ, mức ưu tiên, cam kết hiện có; sau đó xét mức tập trung phù hợp, việc lâu chưa tiến triển và hạn chế phân mảnh. App giải thích thứ tự bằng lý do ngắn, không cần người dùng hiểu điểm số nội bộ.
- Người dùng duyệt từng phương án; backend kiểm tra lại lịch/phiên bản trước khi đặt. Nếu đã thay đổi, trả lại đề xuất mới. Đặt lại cho cùng task cập nhật các khối tương ứng, không thêm bản sao ngoài ý muốn.
- Sự kiện mới gây xung đột tạo yêu cầu xem lại kế hoạch. Không tự dời phiên đã cam kết; nếu phương án được duyệt làm dời việc đã đến hạn thì vẫn áp quy tắc hoãn.

## 7. Pomodoro và ghi nhận thời gian

- Bắt đầu từ task, lịch hoặc màn hình Tập trung. Một phiên gắn với một lần thực hiện; mặc định 25 phút làm/5 phút nghỉ, có thể chỉnh.
- Chỉ một phiên hoạt động trên toàn tài khoản. Chuyển task phải xác nhận lưu phiên cũ; có tiếp quản giữa các thiết bị. Không thay tên task trên cùng bộ đếm đang chạy một cách âm thầm.
- Dùng mốc thời gian, trạng thái bắt đầu/tạm dừng/tiếp tục/kết thúc và thiết bị sở hữu phiên; không dựa vào số lần JavaScript chạy mỗi giây.
- Mất mạng cho phép giữ phiên hiện tại và lưu kết quả chờ đồng bộ. Bắt đầu/tiếp quản phiên mới phải được backend xác nhận. Nếu tiếp quản ở thiết bị khác khi máy cũ mất mạng, kết quả từ máy cũ chỉ được đối chiếu tới thời điểm tiếp quản; không cộng hai khoảng trùng như hai lần lao động.
- Máy ngủ, app đóng bất thường hoặc khoảng không hoạt động dài: yêu cầu xác nhận thời gian thực sự làm thay vì tự tính toàn bộ là tập trung.
- Hết phiên không tự hoàn thành task. Chọn Hoàn thành hoặc cập nhật ước lượng còn lại; thời gian đã làm không tự chuyển thành phần trăm hoàn thành.
- Lưu thời lượng dự kiến ban đầu, thời gian thực tế, ước lượng còn lại, gián đoạn và lịch sử chỉnh sửa riêng. Cho sửa giờ thực tế có lý do; giữ bản ghi gốc.
- Khi trở về từ phiên, hiện bước tiếp theo và lời nghỉ ngắn. Không dùng biểu đồ/vòng tiến độ trang trí thay cho thông tin task.

## 8. Rà soát và báo cáo tuần

### 8.1. Rà soát ngày

Hôm nay đưa ra các cam kết trong ngày, việc quá hạn, lời nhắc chưa phản hồi, thẻ Cần quyết định, Hộp thư đến chưa làm rõ và dự án thiếu bước tiếp theo. Việc tương lai nằm trong lịch/dự án, không làm đầy danh sách Hôm nay nếu chưa cần chú ý.

### 8.2. Báo cáo truy cập được bất kỳ lúc nào

- Desktop có mục **Báo cáo** trên thanh bên. iPhone có **Thêm → Báo cáo**. Chọn tuần hiện tại/tuần cũ, mở báo cáo đã lưu và dữ liệu nguồn; không phụ thuộc vào việc nhận thông báo Chủ nhật.
- Số liệu được backend tính độc lập với AI: việc hoàn thành, cam kết chưa xong, lần lặp lỡ, lượt hoãn/lý do, dự kiến so với thực tế, thời gian theo ngày/dự án/không gian và gián đoạn.
- Lưu lịch sử cam kết để chuyển việc sang tuần sau không xóa việc đã lỡ khỏi tuần trước. Số hoàn thành tính theo lần thực hiện, có thể xem task gốc; không lẫn số chuỗi lặp với số lần lặp. Mỗi phiên thời gian chỉ cộng một lần.
- Các tỷ lệ phải nêu rõ tập công việc và khoảng thời gian dùng để tính; không lấy tất cả task còn mở làm mẫu số mơ hồ. Báo cáo phiên bản cũ giữ mốc dữ liệu; bản mới thể hiện dữ liệu đã đối chiếu/bổ sung.
- Đối chiếu năng lượng tự khai với loại việc/thời gian như quan sát có điều kiện. Thiếu dữ liệu phải ghi rõ; không suy đoán sức khỏe hoặc năng lực, không đưa điểm năng suất tổng hợp để đánh giá con người.

### 8.3. Nhận xét và đề xuất AI

- Chuẩn bị tối Chủ nhật; mặc định **20:00**, Asia/Ho_Chi_Minh. Báo cáo ghi rõ “từ thứ Hai đến thời điểm tạo”, không giả định đã có dữ liệu cả tối Chủ nhật.
- Phần nhận xét gồm điều làm tốt, trở ngại lặp lại và tối đa ba thay đổi cụ thể nên thử. Mỗi nhận xét dẫn về số liệu/lý do liên quan hoặc ghi rõ là giả thuyết.
- Nếu AI lỗi/chưa chạy/không được cấp dữ liệu, số liệu vẫn xem được và có trạng thái giải thích. Khi dữ liệu đến muộn có thể tạo phiên bản mới, không ghi đè âm thầm kết luận cũ.
- Đề xuất như giảm tải, đổi thời lượng, chia task hoặc đổi giờ luôn là bản nháp. Khi duyệt phải hiện thay đổi cụ thể và kiểm tra lại quy tắc hoãn/lịch trên dữ liệu hiện tại.

## 9. Tích hợp AI agent CLI và quyền dữ liệu

- Có giao diện adapter chung: nhận loại tác vụ, ngữ cảnh đã lọc và cấu trúc kết quả mong đợi; trả về bản nháp hoặc lỗi có thể xử lý. Hàng đợi lưu trạng thái Chờ / Đang chạy / Xong / Lỗi / Hủy.
- **Codex CLI là adapter đầu tiên được đề xuất**. Antigravity CLI là adapter mở rộng sau khi xác minh cách gọi không tương tác, đăng nhập, đầu ra và giới hạn thực tế. Không hứa cả hai chạy được trước khi thử.
- Chạy tác vụ trong tiến trình/thư mục riêng với giới hạn thời gian/tài nguyên; chỉ cấp ngữ cảnh được phép. Không cho đọc trực tiếp database, backup, mã khóa, token Drive hoặc thư mục người dùng tùy ý. Không ghép note vào câu lệnh shell.
- Quyền gửi áp dụng tới cả note, task, tên dự án, lý do hoãn, năng lượng và ngữ cảnh/tóm tắt suy ra từ nội dung bị loại trừ. Có mặc định theo dự án và xem trước từng yêu cầu; mặc định chưa gửi khi chưa cho phép.
- Bản gốc được giữ. Kết quả phải qua kiểm tra định dạng; AI không ghi trực tiếp vào dữ liệu chuẩn và không thể vượt quy tắc ba lần hoãn.
- Lưu dấu vết nội dung đã gửi, phiên bản đầu vào và đề xuất được duyệt; phần lưu này cũng chịu quyền truy cập/xóa và không ghi bí mật vào log. Đề xuất trên phiên bản task cũ phải đối chiếu lại trước khi áp dụng.
- App hoạt động đầy đủ khi AI tắt hoặc cần đăng nhập lại. Báo lỗi/cho thử lại, không treo luồng ghi nhanh, nhắc hay Pomodoro.

## 10. Kiến trúc, dữ liệu và đồng bộ

### 10.1. Thành phần

```text
Client Windows ─┐
Client Linux ────┼── Tailscale + HTTPS ── Backend Linux
PWA iPhone ──────┘                       ├─ API + xác thực thiết bị
                                        ├─ dự án/task/lặp/lịch
                                        ├─ nhắc + kênh đồng bộ
                                        ├─ phiên tập trung + báo cáo
                                        ├─ hàng đợi AI + adapter CLI
                                        ├─ sao lưu Google Drive
                                        └─ cơ sở dữ liệu trên ổ cục bộ
```

Một backend dạng modular monolith, chia module nghiệp vụ rõ ràng; không cần microservice cho một người dùng. Quy tắc nằm ở backend, client phụ trách tương tác và tích hợp thiết bị. API không phụ thuộc một giao diện cụ thể để có thể bổ sung frontend sau này.

### 10.2. Lựa chọn công nghệ theo hiệu năng — ứng viên cần đo

Ưu tiên mới: tốc độ chạy và tài nguyên khi dùng thật. Không mặc định React/Node; không tuyên bố một stack nhanh nhất khi chưa có bản release đo trên thiết bị đích. Cần đánh giá đồng thời desktop, iPhone PWA và backend, không so tên ngôn ngữ ở hai lớp khác nhau.

| Phương án giao diện | Lợi thế kiến trúc cần kiểm chứng | Đánh đổi đối với Nhịp |
|---|---|---|
| Tauri + Svelte + TypeScript | UI biên dịch thành JavaScript; desktop dùng WebView hệ điều hành; có thể dùng chung UI với PWA | Vẫn chịu chi phí WebView; phải đo hiệu năng lịch, danh sách và iPhone, không tự coi là nhanh hơn React |
| Tauri + React + TypeScript | Hệ sinh thái UI và lịch rộng; dùng chung UI desktop/PWA | Cần kiểm soát cập nhật state/render và dung lượng phụ thuộc; không giữ làm lựa chọn mặc định |
| Flutter + Dart | Có engine dựng giao diện riêng, hỗ trợ desktop/web; có thể kiểm soát cách dựng UI xuyên nền tảng | Phải kiểm chứng tải lần đầu, bộ nhớ và tích hợp Web Push trên iPhone web; không suy từ bản native sang web |
| Rust + Slint | Có đường dựng UI desktop không qua WebView, Rust và khả năng biên dịch WebAssembly | Cần kiểm chứng lịch tương tác, nhập tiếng Việt, accessibility và iPhone web; nếu phải làm PWA riêng thì tăng chi phí duy trì hai UI |
| C++/QML + Qt Quick | Có đường dựng UI desktop không qua WebView và hệ sinh thái desktop | Đóng gói, module và iPhone web cần thử; Qt cảnh báo mức hỗ trợ mobile browser của WebAssembly cần kiểm chứng. Có thể phải duy trì PWA riêng |

| Backend | Lý do đưa vào so sánh | Lưu ý |
|---|---|---|
| Rust + Axum/Tokio | Biên dịch native; kiểm soát cấp phát và không có tracing GC; phù hợp đánh giá tiến trình thường trực | Build và triển khai phức tạp hơn; không bảo đảm nhanh hơn mọi triển khai Go/Node |
| Go | Biên dịch thành chương trình độc lập, concurrency và công cụ profiling sẵn có | Có GC nhưng không được dùng kết quả Discord năm 2020 để kết luận Go hiện tại chậm; đo cùng bài toán |
| TypeScript + Fastify | Phát triển nghiệp vụ nhanh, hệ sinh thái tích hợp rộng | Có runtime Node; cần tránh chặn event loop và đo CPU/RAM thực tế, không chỉ throughput HTTP |

**Ứng viên ưu tiên để kiểm chứng:** Svelte + TypeScript + Vite cho UI; Tauri/Rust cho desktop; Rust + Axum/Tokio cho backend; SQLite cho dữ liệu. Đây là lựa chọn kỹ thuật đề xuất theo tiêu chí mới, chưa phải kết quả benchmark hay cam kết nhanh nhất.

- Phương án native Rust/Slint là đối chứng khi WebView gây vấn đề tài nguyên; Flutter và Qt được giữ trong bảng lựa chọn, không bị loại chỉ vì đội phát triển quen web.
- Backend Linux và phần Rust trong client Tauri là hai tiến trình ở hai máy khác nhau; không nhầm chúng thành một backend cục bộ duy nhất.
- SQLite nằm trên ổ cục bộ server, client truy cập qua REST JSON có phiên bản; SSE và đối chiếu theo mốc thay đổi phục vụ đồng bộ. Không chia sẻ file database qua mạng.
- Logic nghiệp vụ chuẩn vẫn ở backend. Sinh kiểu API cho frontend từ hợp đồng chung để giảm lệch kiểu Rust/TypeScript; không sao chép quy tắc hoãn thành hai nguồn sự thật.
- Server chạy bằng systemd dưới tài khoản dịch vụ riêng là mặc định đề xuất; AI chạy qua tiến trình được giới hạn. Không cần container hóa chỉ để gọi một CLI.
- Đo release build và tài nguyên đầy đủ trước khi cố định stack; nguyên tắc và bài đo ở mục 17. Các mục chức năng, bảo mật và giới hạn nền tảng vẫn áp dụng nguyên vẹn.

### 10.3. Dữ liệu và giao diện nghiệp vụ

| Nhóm | Dữ liệu chính |
|---|---|
| Tổ chức | Dự án, task, lần thực hiện, quy tắc lặp, note, liên kết tái cấu trúc |
| Kế hoạch | Sự kiện, khối Bận, phiên làm việc, lịch sử cam kết, sức chứa ngày, đề xuất lịch |
| Nhắc | Lượt nhắc, lịch nhắc lại, kết quả gửi theo thiết bị, phản hồi, bộ đếm hoãn |
| Thực tế | Phiên Pomodoro, mốc chuyển thiết bị, gián đoạn, chỉnh sửa thời gian, năng lượng |
| Tổng hợp | Báo cáo tuần có phiên bản/mốc dữ liệu, đề xuất AI, tác vụ AI |
| Vận hành | Thiết bị/phiên xác thực, mốc đồng bộ, nhật ký thay đổi, hàng đợi tác vụ, bản kê sao lưu |

Giao diện cần có các nhóm thao tác: tạo/sửa/lưu trữ dự án; ghi nhận/làm rõ/sửa task và note; sinh/sửa quy tắc lặp; phản hồi lượt nhắc; đề xuất/duyệt lịch; bắt đầu/tạm dừng/kết thúc/tiếp quản phiên; xem báo cáo; duyệt AI; quản lý thiết bị và backup. Không cho client sửa trực tiếp các bộ đếm nghiệp vụ.

### 10.4. Đồng bộ và xung đột

- Lệnh thay đổi có mã chống xử lý lặp, ID thiết bị và phiên bản trạng thái đã đọc. Backend áp quy tắc trong transaction, lưu kết quả lệnh và thay đổi dữ liệu cùng nhau.
- Hai máy Hoãn cùng một lượt nhắc chỉ tính một lần. Hoàn thành rồi nhận lệnh Hoãn cũ không được mở lại task. Với thay đổi khác nhau, lệnh dựa trên phiên bản cũ được yêu cầu đối chiếu lại thay vì ghi đè im lặng.
- Thay đổi dữ liệu và thông điệp cần gửi được lưu bền vững cùng transaction; worker có thể thử gửi lại mà không làm phát sinh phản hồi/phiên/task trùng.
- Mỗi lần lặp, phiên và báo cáo có định danh ổn định. Khởi động lại server phải khôi phục tác vụ tới hạn; không phụ thuộc timer chỉ nằm trong bộ nhớ.
- Khi client quá cũ, báo cần cập nhật. Cập nhật PWA không tự tải lại giữa một bản nháp hoặc phiên tập trung đang dùng.

### 10.5. Ngoại tuyến bản đầu

| Khả năng | Desktop | iPhone PWA |
|---|---|---|
| Xem dữ liệu đã tải | Có | Có nếu cache còn tồn tại |
| Ghi nhanh task/note | Lưu nháp cục bộ, gửi lại | Lưu nháp cục bộ, gửi lại khi mở/kết nối được |
| Nhắc khi server tạm mất kết nối | Theo lịch đã nhận, khi tiến trình client chạy | Không cam kết có nhắc mới; Web Push cần backend gửi |
| Pomodoro đang chạy | Giữ phiên, đợi đối chiếu | Giữ mốc phiên, đối chiếu khi mở lại; không hứa tab chạy nền liên tục |
| Hoàn thành | Có thể ghi phản hồi chờ đồng bộ và dừng nhắc tại máy đó | Tương tự khi app truy cập được dữ liệu cache |
| Hoãn, đặt lịch, sửa task đầy đủ | Yêu cầu kết nối backend để giữ quy tắc toàn tài khoản | Tương tự |

Phản hồi chờ đồng bộ phải có nhãn rõ; các máy khác chỉ dừng nhắc sau khi nhận được trạng thái chuẩn. Trạng thái lưu cục bộ không được hiển thị như đã đồng bộ thành công.

## 11. Truy cập, sao lưu và vận hành

### 11.1. Mã khóa và thiết bị

- Một mã khóa để đăng ký/mở phiên ứng dụng; backend lưu giá trị băm phù hợp, giới hạn thử sai. Thiết bị đã đăng ký dùng phiên xác thực riêng có thể thu hồi, không gửi lại mã khóa cho mọi request.
- Tailscale giới hạn đường vào; backend chỉ công bố qua địa chỉ riêng với HTTPS. Tailscale Serve là phương án reverse proxy đề xuất; không mở cổng công khai/Funnel cho nhu cầu hiện tại.
- PWA và desktop lưu thông tin phiên bằng cơ chế phù hợp nền tảng; khóa màn hình cục bộ, xác thực API và khóa giải mã backup là ba trách nhiệm riêng.
- Danh sách thiết bị có thời điểm hoạt động, quyền nhận nhắc và thu hồi. Dữ liệu bị thu hồi không thể tiếp tục đồng bộ; cache cũ cần được xử lý trong luồng đăng xuất.

### 11.2. Sao lưu Google Drive

- Sao lưu dự án, task/lần lặp, note, lịch/cam kết, nhắc/lý do, Pomodoro, năng lượng, báo cáo và cấu hình nghiệp vụ; không chỉ sao lưu báo cáo AI.
- Tạo snapshot database nhất quán bằng cơ chế backup của database; không sao chép mù file đang ghi. Đưa kèm phiên bản schema/app và bản kê kiểm tra toàn vẹn.
- Mã hóa trước khi tải lên thư mục riêng trên Drive. Khóa khôi phục được giữ ngoài PC; không để khóa giải mã ngay cạnh bản mã hoặc lẫn với mã mở app ngắn.
- Token Drive/CLI và bí mật xác thực không nằm ở dạng rõ trong bản xuất nghiệp vụ. Quy trình khôi phục phải liệt kê phần cần đăng nhập/cấp quyền lại và đăng ký lại thiết bị, không giả định các token sẽ tự sống lại.
- Mặc định sao lưu mỗi ngày lúc 02:00, giữ 7 bản theo ngày, 4 bản theo tuần, 3 bản theo tháng. Lỗi tải lên được thử lại và cảnh báo; thiếu dung lượng hoặc hết quyền phải thấy được trong app. Không xóa bản cũ khi chưa có bản mới đã kiểm tra thành công.
- Google OAuth cần cấu hình cho tác vụ không có người thao tác; không hứa quyền sẽ tồn tại vĩnh viễn. Có luồng kết nối lại tài khoản khi quyền hết hiệu lực.
- Khôi phục vào vùng dữ liệu mới, kiểm tra trước khi thay dữ liệu đang dùng; sau khôi phục đối chiếu task tới hạn và tránh phát lại mọi thông báo cũ.

### 11.3. Chẩn đoán, nâng cấp và phục hồi

- Màn hình Trạng thái hiển thị server/kết nối, đồng bộ gần nhất, lời nhắc thử, worker lặp/nhắc, phiên AI, lần sao lưu thành công/thất bại gần nhất.
- Log có ID tác vụ/lệnh để truy lỗi nhưng không ghi token, mã khóa hoặc toàn văn note mặc định. Không thu thập phân tích người dùng bên ngoài khi chưa được yêu cầu.
- Database có migration theo phiên bản; sao lưu trước nâng cấp schema. Backend tự chạy lại sau reboot, client xử lý server khởi động chậm một cách rõ ràng.
- Có kiểm thử khôi phục thật trên môi trường sạch; việc tạo được file backup chưa đủ để nghiệm thu khả năng khôi phục.

## 12. Màn hình và tiêu chí giao diện

- **Hôm nay:** cam kết cần chú ý, năng lượng hiện tại, tải ngày theo không gian, lời nhắc chưa xử lý, Cần quyết định, Hộp thư đến và phiên tập trung.
- **Dự án:** tạo/sửa, lọc công ty/cá nhân, task đúng dự án, thêm task và bước tiếp theo.
- **Lịch:** tháng/tuần/ngày, khối bận, khối cam kết và đề xuất có giải thích; thao tác kéo lịch vẫn tuân thủ hoãn.
- **Ghi nhanh:** task/note với bản nháp bền vững; form đầy đủ mở thêm dự án, thời lượng, tập trung, hạn, lặp và nhắc khi cần.
- **Chi tiết task:** lịch sử hoãn/lý do, lịch lặp, thời gian còn lại/thực tế, note và xử lý trạng thái.
- **Tập trung:** một task, bộ đếm rõ, tạm dừng/kết thúc/tiếp quản và lịch sử phiên.
- **Báo cáo:** tuần hiện tại/lịch sử, số liệu nguồn, nhận xét AI và đề xuất chờ duyệt.
- **Cài đặt/Trạng thái:** giờ làm, sức chứa, nhắc, thiết bị, AI, sao lưu/khôi phục.

Desktop dùng thanh bên; iPhone dùng Hôm nay / Lịch / Dự án / Thêm ở thanh dưới. Ghi chú, Báo cáo và Cài đặt có trong Thêm; Ghi nhanh luôn dễ tiếp cận. Tránh để một tính năng đã có trong đặc tả nhưng không có lối vào.

Hướng thị giác lấy demo Nhịp v2 làm tham chiếu, không bê nguyên các thiếu sót demo: font hệ thống, nền trung tính, màu xanh cho hành động/đang chọn, bố cục thoáng, ít hiệu ứng, hỗ trợ sáng/tối theo hệ thống. Nội dung và số liệu thật quyết định hiển thị; không có thống kê tĩnh giả như “đã đồng bộ”.

Tiêu chí thực dụng:

- Ghi nhanh từ một nút và một ô nhập; có thể lưu mà không điền các trường nâng cao.
- Bắt đầu Pomodoro trực tiếp từ task. Không có thao tác chỉ xuất hiện khi rê chuột.
- Vùng chạm mobile khoảng 44×44 px, chữ nội dung dễ đọc; mục phụ không bị dùng cỡ chữ quá nhỏ để nhét vừa.
- Có trạng thái tải, trống, lỗi, mất mạng, thiếu quyền và thành công; lỗi nói rõ cách sửa. Có Hoàn tác cho thao tác dễ bấm nhầm.
- Điều hướng có tên, hỗ trợ bàn phím, focus rõ, form có nhãn và báo lỗi; hộp thoại quản lý focus, Escape và trả focus. Trạng thái không chỉ phân biệt bằng màu.
- Thử ở iPhone thực, desktop Windows/Linux và độ rộng 320/390/768/1024 px; chữ dài/zoom không làm mất thao tác.

## 13. Tiêu chí nghiệm thu và kịch bản quan trọng

| ID | Kịch bản phải đạt |
|---|---|
| AT01 | Tạo dự án công ty/cá nhân; mở đúng task; tạo task trong dự án trên cả ba client |
| AT02 | Ghi nhanh khi mất mạng, đóng/mở form, đồng bộ lại không mất chi tiết và không tạo trùng |
| AT03 | Task/note và thay đổi xuất hiện trên máy khác; ghi chú mở đúng nội dung; Hoàn tác có đối chiếu |
| AT04 | Nhắc mọi thiết bị, im lặng vẫn nhắc mỗi 3 giờ; gom thông báo không làm mất mục |
| AT05 | Lịch hẹn lần hoãn thứ ba vẫn hiệu lực; khi tới hạn bắt buộc quyết định; từ chối lần thứ tư qua nút/kéo lịch/API |
| AT06 | Bắt đầu nhưng không xong trở về quyết định khi hết lượt; tái cấu trúc giữ quan hệ và lịch sử |
| AT07 | Hai máy Hoãn cùng lượt chỉ tăng một; Hoàn thành rồi nhận lệnh cũ không sống lại task |
| AT08 | Phân biệt server gửi push thành công với người đã phản hồi; quyền tắt/lỗi push có thể chẩn đoán |
| AT09 | Server reboot/máy ngủ/mất mạng không sinh trùng lượt; desktop cache và giới hạn iPhone đúng như mô tả |
| AT10 | Lần lặp quá hạn vẫn tồn tại, lần mới vẫn sinh; xử lý nhóm có lý do, không vượt giới hạn từng lần |
| AT11 | Sửa một lần/tương lai, ngày cuối tháng, cuối tuần, tạm dừng/tiếp tục không sửa lịch sử hoặc tạo trùng |
| AT12 | Tìm lịch đúng giờ công ty/cá nhân, trừ Bận/nghỉ, dùng sức chứa và thời lượng còn lại; không xếp việc Chờ |
| AT13 | Không xếp vừa có lý do; chọn năng lượng chỉ đổi đề xuất; duyệt lịch cũ phải đối chiếu; không đặt trùng |
| AT14 | Một Pomodoro hoạt động, chuyển thiết bị có xác nhận, mất mạng/ngủ/tiếp quản không cộng trùng; hết phiên không tự xong task |
| AT15 | Thời lượng ban đầu/thực tế/còn lại riêng; chỉnh thời gian có dấu vết; báo cáo không lấy phút làm phần trăm hoàn thành |
| AT16 | Báo cáo truy cập được, chọn tuần cũ, mở nguồn; AI tắt vẫn xem số liệu; mốc tối Chủ nhật được ghi rõ |
| AT17 | Nội dung bị loại trừ không lọt vào ngữ cảnh AI; bản gốc giữ nguyên; đề xuất cũ/mới đều phải duyệt và kiểm tra quy tắc |
| AT18 | Khôi phục bản mã Drive trên Linux sạch, giữ đúng task/lịch/note/Pomodoro/năng lượng/báo cáo; đăng ký lại kết nối cần thiết |
| AT19 | Thu hồi thiết bị, hết quyền Drive, worker lỗi và client cũ đều có thông báo/trạng thái xử lý |
| AT20 | Giao diện iPhone clean, có đủ lối vào Dự án mới/Báo cáo/Năng lượng, dùng được bàn phím và vùng chạm/độ rộng yêu cầu |

Kiểm thử gồm quy tắc nghiệp vụ, tích hợp database/worker và kiểm thử đầu-cuối các luồng trên client. Thông báo iPhone, bộ cài desktop và khôi phục cần thử thật; kiểm tra mã nguồn hoặc demo không thay được các bước này.

## 14. Đối chiếu đủ 18 góp ý

| # | Nội dung rà soát | Nơi đã giải quyết | Kiểm chứng |
|---|---|---|---|
| 1 | Thời điểm hết ba lượt hoãn | 4.2: lịch hẹn thứ ba còn hiệu lực, quyết định khi đến lịch | AT05 |
| 2 | Định nghĩa hành động tính hoãn | 4.2: nút/kéo/sửa, trước và sau khi đến hạn | AT05, AT07 |
| 3 | Né qua Bắt đầu/chia nhỏ | 3.2, 4.2: điểm kiểm tra, quan hệ gốc–mới và lịch sử | AT06 |
| 4 | Im lặng khác hoãn | 4.1: bộ đếm riêng; nhắc tiếp 3 giờ theo lựa chọn người dùng | AT04 |
| 5 | Bão thông báo nhiều máy | 4.3: gom, ID ổn định, một cửa sổ nổi, không chiếm bàn phím | AT04, AT08 |
| 6 | Ngoại tuyến/giới hạn iPhone | 4.3, 10.5: cam kết riêng theo client | AT09 |
| 7 | Xung đột đồng bộ | 10.4: phiên bản, chống lặp và transaction | AT07 |
| 8 | Tồn đọng task lặp | 5: giữ từng lần, nhóm và xử lý nhiều lần có lý do | AT10 |
| 9 | Sửa/kết thúc chuỗi lặp | 5: phạm vi sửa, ngày tháng, tạm dừng và múi giờ | AT11 |
| 10 | Ghi nhanh bị chặn bởi dự án/mạng | 3.3, 10.5: Hộp thư đến, bản nháp, gửi lại | AT02 |
| 11 | Sức chứa và nghỉ chưa rõ | 1.1, 6.1–6.2: trần đã chọn, nghỉ và năng lượng | AT12, AT13 |
| 12 | Lịch trống thiếu bận/điều kiện | 3.2, 6.1–6.3: Bận, Chờ, ngày có thể bắt đầu | AT12 |
| 13 | Không có lịch khả thi | 6.3: Không xếp vừa, giải thích, duyệt lại | AT13 |
| 14 | Thời gian không phải tiến độ | 7: ba trường thời lượng, cập nhật còn lại | AT15 |
| 15 | Pomodoro đếm trùng | 7, 10.4–10.5: một phiên, tiếp quản, đối chiếu ngoại tuyến | AT14 |
| 16 | AI rò qua ngữ cảnh | 8–9: lọc đầu vào liên quan, bằng chứng và quyền dữ liệu | AT16, AT17 |
| 17 | CLI chưa kiểm chứng/phạm vi lớn | 2, 9: chia mốc, một adapter đầu tiên, kiểm chứng công cụ thứ hai | AT17 và cổng kiểm chứng |
| 18 | UI/độ tin cậy chưa đo được | 11–13: phong cách iPhone, trạng thái thật, lối vào và tiêu chí thao tác | AT18–AT20 |

## 15. Quyết định còn mở và cổng kiểm chứng trước xây dựng

### 15.1. Cần làm rõ trước khi xây đầy đủ

1. **Stack qua kiểm chứng:** người dùng ưu tiên tốc độ chạy và yêu cầu mở rộng lựa chọn. Dùng ứng viên Svelte/Tauri + Rust/Axum + SQLite ở mục 10.2 để đo đầu tiên, có đối chứng thay vì yêu cầu người dùng tự chọn theo danh tiếng công nghệ. Chỉ cố định sau bài đo mục 17; systemd là cách chạy server đề xuất ban đầu.
2. **Thiết bị đích cụ thể:** phiên bản Windows, bản phân phối/phiên bản Linux cho server và client, phiên bản iOS. Dùng thông tin này để quyết định nền build, định dạng bộ cài và giới hạn phải hỗ trợ; không tự suy ra PC đang chạy công cụ chính là server sẽ triển khai.
Thứ tự triển khai mặc định theo mục 2: kiểm chứng một luồng thật trên cả Windows/Linux/iPhone, rồi hoàn thiện đầy đủ. Đây là thứ tự làm việc, không phải quyết định cắt tính năng và không cần hỏi lại nếu người dùng không yêu cầu đổi.

### 15.2. Cần kiểm chứng bằng thử nghiệm kỹ thuật nhỏ trước khi hứa khả năng

- Cài PWA trên iPhone thật, cấp Web Push, thử app đóng/Focus/mất Tailscale/chạm thông báo và đối chiếu khi VPN về. Nếu không đạt nhu cầu nhắc mạnh, phải xem lại kênh thông báo hoặc phương án iPhone trước khi đi sâu UI.
- Đóng gói/chạy nền/thông báo/tự khởi động của Tauri trên Windows và bản Linux đích; không suy từ trình duyệt sang hành vi ứng dụng cài đặt.
- Codex CLI chạy được dưới tài khoản backend, đầu ra có cấu trúc, phiên đăng nhập và lỗi/timeout được kiểm soát. Antigravity cần kiểm chứng riêng nếu đưa vào cùng lần phát hành.
- Thiết lập Google Drive OAuth cho sao lưu tự động và thử tải/khôi phục file mã hóa. Thư mục Drive và tài khoản cụ thể sẽ chọn khi cấu hình, không cần đặt bí mật trong đặc tả.

### 15.3. Cần cấu hình khi thiết lập, không chặn việc bắt đầu code

Mã khóa và khóa khôi phục; địa chỉ Tailscale của server; tài khoản/thư mục Drive; tài khoản CLI và hạn mức AI; cho phép gửi AI theo dự án; giờ báo cáo 20:00 và lịch backup 02:00; tinh chỉnh Pomodoro/nghỉ. Không cần cung cấp mã khóa hoặc token trong chat.

Tên Nhịp là tên demo đang dùng, có thể đổi mà không ảnh hưởng kiến trúc. Các demo cũ giữ làm tham chiếu, không tự sửa khi cập nhật đặc tả.

## 16. Nguồn kỹ thuật tham khảo

Nguồn xác nhận khả năng công nghệ, không thay thế kiểm chứng trên thiết bị thật:

- [React + Vite](https://react.dev/learn/build-a-react-app-from-scratch)
- [Fastify TypeScript](https://fastify.dev/docs/latest/Reference/TypeScript/)
- [Tauri và cách phân phối](https://v2.tauri.app/distribute/)
- [SQLite trên application server](https://sqlite.org/whentouse.html)
- [Snapshot SQLite](https://sqlite.org/backup.html)
- [Tailscale Serve](https://tailscale.com/docs/reference/tailscale-cli/serve)
- [Web Push cho iPhone Home Screen](https://webkit.org/blog/13878/)
- [Google OAuth tác vụ nền](https://developers.google.com/identity/protocols/oauth2/web-server)
- [Drive upload](https://developers.google.com/workspace/drive/api/guides/manage-uploads)

- [Svelte compiler](https://svelte.dev/docs/svelte/svelte-compiler)
- [Flutter architecture](https://docs.flutter.dev/resources/architectural-overview)
- [Slint platforms và WebAssembly](https://docs.slint.dev/latest/docs/slint/guide/platforms/web/)
- [Qt và giới hạn mobile browser](https://doc.qt.io/qt-6.10/supported-platforms.html)
- [Axum](https://docs.rs/axum/latest/axum/)
- [Go GC và đánh đổi hiệu năng](https://go.dev/doc/gc-guide)

## 17. Cổng đo hiệu năng trước khi cố định stack

Đã hoàn tất lượt đo release đầu tiên cho lát cắt Svelte/Tauri + Rust trên Linux; xem [báo cáo thực nghiệm](prototype/PERFORMANCE_REPORT.md) và [dữ liệu gốc](prototype/results/linux-release.json). Chưa đo Windows và iPhone; không lấy kết quả Linux hoặc demo HTML để suy ra các nền tảng còn lại.

### 17.1. Bài đo và điều kiện công bằng

- Dùng cùng chức năng/dữ liệu: ghi nhanh, Hôm nay, lịch tuần, cập nhật đồng bộ, Pomodoro và báo cáo số liệu. Khả năng nhập tiếng Việt, accessibility, nhắc nền và PWA là điều kiện bắt buộc, không đánh đổi lấy một số benchmark đẹp.
- Bộ dữ liệu kiểm tra: 100 dự án, 10.000 lần thực hiện cùng lịch sử, 4 client; chỉ render phần cần xem. Có kịch bản lịch dày, đồng bộ dồn và chạy báo cáo trong lúc tới hạn nhắc.
- Dùng release build, cùng thiết bị, cùng dữ liệu và cùng điều kiện cache/network. Ghi rõ phiên bản runtime, trình duyệt/WebView, hệ điều hành, phần cứng và cấu hình nguồn điện.
- Đo riêng startup nguội/ấm, phản hồi UI cục bộ, thời gian mạng Tailscale, xử lý backend và thời gian AI bên ngoài. Không cộng tất cả thành một chỉ số rồi quy lỗi cho framework.
- Báo median/p95 từ ít nhất 20 lần cho startup/thao tác chính và mẫu đủ lớn cho API; xem CPU/RAM lúc nghỉ, lúc dùng và sau khi đóng cửa sổ vào khay.
- Desktop tính cả tiến trình con/WebView, không chỉ Rust host. Không so trực tiếp RSS/PSS/working set khác định nghĩa giữa OS; ghi rõ phép đo và so ứng viên trên cùng nền tảng.

### 17.2. Mục tiêu khởi đầu, cần hiệu chỉnh theo máy đích

- Phản hồi UI từ dữ liệu đã có: p95 dưới 100 ms.
- Desktop mở nguội tới khi Hôm nay thao tác được: mục tiêu dưới 2 giây; không để kết nối backend chặn việc mở và ghi nhanh cục bộ.
- API đọc/ghi task cơ bản ở server cục bộ: p95 dưới 50 ms, tách khỏi độ trễ mạng.
- Tìm giờ cho một task trong lịch 7 ngày: p95 dưới 200 ms trên dữ liệu kiểm tra; tác vụ báo cáo/AI không chặn đường nhắc và API.
- CPU lúc nghỉ phải gần không, không polling bận; chỉ cập nhật đồng hồ khi cần. Ghi lại mức RAM release ban đầu rồi đặt ngân sách trên phần cứng thật; chưa cam kết một con số MB chung cho mọi OS.

### 17.3. Quy tắc chọn và kiểm soát phạm vi thử nghiệm

- Thử ứng viên Svelte/Tauri + Rust trước bằng một lát cắt dọc nhỏ, không xây hai ứng dụng đầy đủ để so sánh.
- Nếu đạt chức năng và ngân sách, giữ ứng viên này để triển khai. Điều đó có nghĩa đạt yêu cầu đã đo, không đồng nghĩa đã chứng minh nhanh nhất trong mọi lựa chọn.
- Nếu không đạt, profile để xác định phần chậm: UI/render, WebView, database, mạng hay lịch/worker. Chỉ thay lớp gây vấn đề, tránh viết lại toàn bộ theo cảm tính.
- Khi cần đối chứng UI, dựng cùng lát cắt trên Slint hoặc Flutter theo nút thắt đã đo; React/Tauri vẫn là đối chứng nếu thành phần lịch hiện có giúp giảm chi phí mà không vượt ngân sách.
- Nếu muốn khẳng định nhanh nhất trong nhóm shortlist, phải đo trực tiếp các ứng viên đó. Tài liệu chỉ hỗ trợ đánh giá khả năng và đánh đổi, không cung cấp bảng xếp hạng hiệu năng cho app chưa xây.

### 17.4. Trạng thái thực nghiệm Linux

- Với 100 dự án/10.000 lần thực hiện ban đầu: p95 đọc hôm nay 1,40 ms, ghi task 2,51 ms, tìm lịch 1,24 ms trở xuống; bốn client hỗn hợp 6,86 ms. Đây là round trip HTTP loopback, chưa gồm Tailscale.
- Tauri release mở 20 lần thành công, p95 1,684 giây khi cache ấm; chưa xác minh mục tiêu mở nguội. Proxy render đổi màn hình p95 48 ms, chọn task 33 ms, chưa phải INP thao tác thật.
- PSS toàn cây desktop trung vị 110,33 MiB; backend 6,94 MiB. CPU desktop nghỉ 10,45% một nhân trong 60 giây: chưa đạt yêu cầu gần không, cần profile trước khi cố định stack.
- Giữ Svelte/Tauri + Rust làm ứng viên đang kiểm chứng. Các phần nhắc nền, task lặp đầy đủ, xác thực, AI, Drive và đồng bộ vẫn phải triển khai và kiểm tra; kết quả prototype không thay thế tiêu chí nghiệm thu sản phẩm.
