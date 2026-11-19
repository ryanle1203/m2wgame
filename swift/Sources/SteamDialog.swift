import AppKit
import SwiftUI

/// Shows Steam's real (otherwise hidden) window inside M2WGame so the user can read and answer
/// its dialogs: clicks on the picture are passed to Steam at the matching position.
struct SteamDialogSheet: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss
    let game: GameItem
    @State private var image: NSImage?
    @State private var problem: String?
    @State private var text = ""

    // Steam's main window is a 1280 x 800 page.
    private let pageW = 1280.0
    private let pageH = 800.0

    private var live: GameItem? { store.snap?.games.first(where: { $0.id == game.id }) }

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Hộp thoại của Steam: \(game.name)").font(.headline)
                Spacer()
                if live?.assist != true {
                    HStack(spacing: 6) {
                        ProgressView().controlSize(.small)
                        Text("Đang mở lại Steam…").font(.caption).foregroundStyle(.secondary)
                    }
                }
            }
            .padding(14)
            Divider()
            GeometryReader { geo in
                let scale = min(geo.size.width / pageW, geo.size.height / pageH)
                let w = pageW * scale, h = pageH * scale
                ZStack {
                    Color.black
                    if let image {
                        Image(nsImage: image)
                            .resizable()
                            .frame(width: w, height: h)
                            .contentShape(Rectangle())
                            .gesture(
                                DragGesture(minimumDistance: 0).onEnded { v in
                                    click(x: v.location.x / scale, y: v.location.y / scale)
                                }
                            )
                    } else {
                        Text(problem ?? L("Đang chờ Steam mở…"))
                            .foregroundStyle(.secondary)
                            .padding()
                    }
                }
                .frame(width: geo.size.width, height: geo.size.height)
            }
            .frame(minHeight: 480)
            Divider()
            HStack(spacing: 8) {
                Text("Bấm trực tiếp lên hình, như trên cửa sổ Steam thật.")
                    .font(.caption).foregroundStyle(.secondary)
                Spacer()
                Button { scroll(-400) } label: { Image(systemName: "chevron.up") }
                    .help("Cuộn lên")
                Button { scroll(400) } label: { Image(systemName: "chevron.down") }
                    .help("Cuộn xuống")
                TextField("Gõ chữ vào ô đang chọn", text: $text)
                    .frame(width: 180)
                    .onSubmit(sendText)
                Button("Gửi", action: sendText).disabled(text.isEmpty)
                Button("Mở lại game") {
                    Task { await store.send(["cmd": "steam_assist_relaunch", "id": game.id]) }
                }
                .help("Dùng nếu sau khi bạn trả lời, Steam không tự mở game")
                Button("Đóng") { dismiss() }
                    .keyboardShortcut(.defaultAction)
                    .buttonStyle(.borderedProminent)
            }
            .padding(12)
        }
        .frame(width: 900)
        .task { await refreshLoop() }
    }

    private func refreshLoop() async {
        while !Task.isCancelled {
            if live?.assist == true {
                if let data = await Bridge.call(["cmd": "steam_ui_shot", "id": game.id]),
                   let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
                    if let b64 = obj["image"] as? String, let d = Data(base64Encoded: b64), let img = NSImage(data: d) {
                        image = img
                        problem = nil
                    } else {
                        problem = obj["error"] as? String
                    }
                }
            }
            try? await Task.sleep(for: .milliseconds(900))
        }
    }

    private func click(x: Double, y: Double) {
        guard x >= 0, y >= 0, x <= pageW, y <= pageH else { return }
        Task { _ = await Bridge.call(["cmd": "steam_ui_click", "id": game.id, "x": x, "y": y]) }
    }

    private func scroll(_ dy: Double) {
        Task { _ = await Bridge.call(["cmd": "steam_ui_scroll", "id": game.id, "x": 640.0, "y": 400.0, "dy": dy]) }
    }

    private func sendText() {
        let t = text
        guard !t.isEmpty else { return }
        text = ""
        Task { _ = await Bridge.call(["cmd": "steam_ui_text", "id": game.id, "text": t]) }
    }
}
