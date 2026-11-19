# M2WGame

[English](../../README.md) · **Tiếng Việt** · [简体中文](README.zh-CN.md) · [Español](README.es.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Português (BR)](README.pt-BR.md) · [Français](README.fr.md) · [Deutsch](README.de.md)

Ứng dụng Mac giúp bạn **chơi game Windows trên Mac chip Apple (M1, M2, M3...)**. Nó quản lý các
"môi trường" Windows ảo (Wine), động cơ chạy game, Steam cho Windows và thư viện game của bạn, và
giải thích lỗi bằng lời thường thay vì để bạn đọc một đống log.

> **Trạng thái: bản thử nghiệm sớm.** Có game chạy được, có game không. Hãy đọc phần

> **Phạm vi:** M2WGame tập trung giải quyết một vấn đề: chạy **game Windows mua trên Steam**. Các nhà cung cấp và cửa hàng khác (Epic, GOG, EA, Ubisoft, Battle.net, itch.io…) chưa được hỗ trợ. Bạn vẫn có thể thêm thủ công một file `.exe` riêng lẻ, nhưng cách này chưa được kiểm thử.
> [Giới hạn](#giới-hạn) trước khi mất công với một game. Giao diện có **tiếng Việt và tiếng Anh** (đổi ở thanh bên);
> các ngôn ngữ khác rất được hoan nghênh. Bản chuẩn bằng tiếng Anh: [README.md](../../README.md).

M2WGame **không liên kết với** Apple, Valve, CodeWeavers hay dự án Wine. Nó không kèm theo,
không tải lậu và không vượt qua bảo vệ bản quyền nào: bạn cần sở hữu game hợp pháp.

## Làm được gì

- **Thư viện game**: Chơi / Dừng, cài đặt riêng cho từng game, xem log và gợi ý lỗi dễ hiểu.
- **Môi trường**: tạo, đổi tên, xóa, chọn động cơ cho từng môi trường, cài Steam cho Windows bằng một cú bấm.
- **Động cơ**: tự tìm các bản Wine bạn đặt trong thư mục `Runtimes/` (ví dụ bản Gcenx của Wine Staging hoặc Game Porting Toolkit).
- **Steam không cần cửa sổ Steam** (cửa sổ Steam bị đen khi chạy qua Wine trên Mac):
  thêm game đã có trong Steam bằng một cú bấm; tải game mới bằng SteamCMD ngay trong app (mật khẩu
  và mã Steam Guard chỉ gửi thẳng cho SteamCMD, không lưu); hiện các hộp thoại ẩn của Steam
  (điều khoản, đồng bộ đám mây) ngay trong app để bạn tự trả lời; cảnh báo game 32-bit; gỡ game
  để lấy lại dung lượng.
- **Kiểm tra hệ thống và mạng**: Rosetta, phiên bản macOS, RAM, dung lượng trống, kết nối tới máy chủ Steam.

## Tải về và cài đặt

1. Tải file `M2WGame-<phiên bản>-arm64.dmg` ở trang [Releases](https://github.com/ryanle1203/m2wgame/releases) (Mac chip Apple, macOS 14 trở lên).
2. Mở file `.dmg`, kéo **M2WGame** vào thư mục **Applications**.
3. **Lần mở đầu tiên:** app chưa được Apple công chứng (cần tài khoản nhà phát triển trả phí), nên macOS có thể báo "không thể mở vì không xác minh được nhà phát triển". Cách mở: bấm chuột phải vào app, chọn **Mở (Open)**, rồi **Mở** lần nữa. Hoặc vào *Cài đặt hệ thống → Quyền riêng tư & Bảo mật* và bấm **Vẫn mở (Open Anyway)**.
4. Tự chuẩn bị một bản Wine (xem phần dưới); M2WGame không kèm theo.

## Cài đặt từ mã nguồn

Cần: Mac chip Apple, macOS 14 trở lên, Rosetta 2, Xcode Command Line Tools (`xcode-select --install`) và [Rust](https://rustup.rs).

```bash
git clone <địa chỉ kho mã này>
cd m2wgame
./scripts/build-swift.sh
open target/M2WGame.app
```

Bạn cần tự đặt một bản Wine vào `~/Library/Application Support/M2WGame/Runtimes/` (M2WGame không kèm theo), rồi bấm **Quét lại** ở mục Động cơ. Chi tiết và đường dẫn tải: xem [README.md](../../README.md#get-an-engine).

## Giới hạn

- **Game 32-bit thường không chạy được** trên Wine miễn phí với Mac chip Apple (sập ngay lúc mở). M2WGame sẽ cảnh báo.
- Giao diện Steam không dùng được trong Wine trên Mac (cửa sổ đen). M2WGame có cách vòng như mô tả ở trên.
- Game DirectX 12 và game có chống gian lận phần lớn chưa chạy được.
- Mới thử trên macOS 27, máy M1 Pro. Rất mong phản hồi từ các máy khác.

## Đóng góp, giấy phép

Xem [CONTRIBUTING.md](../../CONTRIBUTING.md) (có vài nguyên tắc cứng về mật khẩu và bản quyền),
[SECURITY.md](../../SECURITY.md) và giấy phép [MIT](../../LICENSE).
