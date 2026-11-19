import SwiftUI

struct EnvironmentsView: View {
    @Environment(Store.self) private var store
    @State private var creating = false
    @State private var renaming: BottleItem?
    @State private var removing: BottleItem?

    private var bottles: [BottleItem] { store.snap?.bottles ?? [] }

    var body: some View {
        Form {
            Section {
                Text("Môi trường là một \"ổ C ảo\" riêng cho Windows. Mỗi môi trường có Steam, game và cài đặt riêng, nên game này không làm hỏng game kia.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
            }
            ForEach(bottles) { bottle in
                BottleSection(bottle: bottle,
                              onRename: { renaming = bottle },
                              onRemove: { removing = bottle })
            }
        }
        .formStyle(.grouped)
        .navigationTitle("Môi trường")
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    creating = true
                } label: {
                    Label("Tạo môi trường", systemImage: "plus")
                }
            }
        }
        .sheet(isPresented: $creating) {
            NameSheet(title: "Môi trường mới",
                      message: "Đặt tên dễ nhớ, ví dụ \"Steam Wine mới\".",
                      actionTitle: "Tạo") { name in
                Task { await store.send(["cmd": "create_bottle", "name": name]) }
            }
        }
        .sheet(item: $renaming) { bottle in
            NameSheet(title: "Đổi tên môi trường", message: "", actionTitle: "Lưu", initial: bottle.name) { name in
                Task { await store.send(["cmd": "rename_bottle", "id": bottle.id, "name": name]) }
            }
        }
        .alert("Xóa môi trường \(removing?.name ?? "")?",
               isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } }),
               presenting: removing) { bottle in
            Button("Xóa vĩnh viễn", role: .destructive) {
                Task { await store.send(["cmd": "remove_bottle", "id": bottle.id]) }
            }
            Button("Hủy", role: .cancel) {}
        } message: { _ in
            Text("Toàn bộ thư mục của môi trường này (Windows ảo, Steam, game đã tải bên trong) sẽ bị xóa khỏi máy và không khôi phục được.")
        }
    }
}

struct BottleSection: View {
    @Environment(Store.self) private var store
    let bottle: BottleItem
    let onRename: () -> Void
    let onRemove: () -> Void

    private var runtimes: [RuntimeItem] { store.snap?.runtimes ?? [] }

    private var defaultRuntimeName: String {
        runtimes.first(where: \.active)?.name ?? "chưa có"
    }

    var body: some View {
        Section {
            LabeledContent("Trạng thái") { stateView }

            Picker("Động cơ", selection: Binding(
                get: { bottle.runtime ?? "" },
                set: { new in
                    var cmd: [String: Any] = ["cmd": "set_bottle_runtime", "id": bottle.id]
                    if !new.isEmpty { cmd["runtime"] = new }
                    Task { await store.send(cmd) }
                })) {
                Text("Mặc định (\(defaultRuntimeName))").tag("")
                ForEach(runtimes) { r in
                    Text(r.version.map { "\(r.name) · \($0)" } ?? r.name).tag(r.path)
                }
                if let current = bottle.runtime, !runtimes.contains(where: { $0.path == current }) {
                    Text("Động cơ đã bị gỡ").tag(current)
                }
            }
            if bottle.effectiveOld {
                Label("Động cơ này khá cũ (Wine 7). Steam cho Windows có thể bị treo; dùng Wine Staging 11 cho Steam.",
                      systemImage: "exclamationmark.triangle.fill")
                    .font(.caption)
                    .foregroundStyle(.orange)
            }

            LabeledContent("Thư mục") {
                HStack {
                    Text(bottle.path)
                        .font(.caption)
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .foregroundStyle(.secondary)
                        .textSelection(.enabled)
                    Button("Hiện") { reveal(bottle.path) }
                }
            }

            LabeledContent("Steam") { steamView }

            HStack {
                Text("\(bottle.gameCount) game dùng môi trường này")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Spacer()
                Button("Đổi tên…", action: onRename)
                Button("Xóa…", role: .destructive, action: onRemove)
            }
        } header: {
            Text(bottle.name)
        }
    }

    @ViewBuilder
    private var stateView: some View {
        if bottle.busy {
            HStack(spacing: 6) {
                ProgressView().controlSize(.small)
                Text("Đang khởi tạo (vài phút)…")
            }
        } else if bottle.initialized {
            Label("Sẵn sàng", systemImage: "checkmark.circle.fill")
                .foregroundStyle(.green)
        } else {
            HStack {
                Text("Chưa khởi tạo").foregroundStyle(.secondary)
                Button("Khởi tạo ngay") {
                    Task { await store.send(["cmd": "init_bottle", "id": bottle.id]) }
                }
            }
        }
    }

    @ViewBuilder
    private var steamView: some View {
        if bottle.installingSteam {
            HStack(spacing: 6) {
                ProgressView().controlSize(.small)
                Text("Đang cài Steam…")
            }
        } else if bottle.steamInstalled {
            HStack {
                Label("Đã cài", systemImage: "checkmark.circle.fill").foregroundStyle(.green)
                if bottle.steamInLibrary {
                    Text("· đã có trong thư viện").foregroundStyle(.secondary)
                } else {
                    Button("Thêm vào thư viện") {
                    Task {
                        if await store.send(["cmd": "add_steam", "bottle_id": bottle.id]) {
                            store.section = .library
                        }
                    }
                    }
                }
            }
        } else {
            HStack {
                Text("Chưa cài").foregroundStyle(.secondary)
                Button("Cài Steam…") {
                    Task { await store.send(["cmd": "install_steam", "bottle_id": bottle.id]) }
                }
            }
        }
    }
}
