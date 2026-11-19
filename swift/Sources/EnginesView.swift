import SwiftUI

struct EnginesView: View {
    @Environment(Store.self) private var store

    private var runtimes: [RuntimeItem] { store.snap?.runtimes ?? [] }

    var body: some View {
        Group {
            if store.snap == nil {
                ProgressView()
            } else if runtimes.isEmpty {
                ContentUnavailableView {
                    Label("Chưa có động cơ", systemImage: "cpu")
                } description: {
                    Text("Cần một bản Wine để chạy game Windows. Tải bản Gcenx (Game Porting Toolkit hoặc Wine Staging), giải nén vào thư mục Runtimes, rồi bấm Quét lại.")
                } actions: {
                    HStack {
                        Button("Mở thư mục Runtimes") { openRuntimesFolder() }
                        Button("Quét lại") { rescan() }
                            .buttonStyle(.borderedProminent)
                    }
                }
            } else {
                Form {
                    Section {
                        Text("Động cơ là phần mềm dịch lệnh Windows sang Mac. Game cũ thường hợp Wine Staging; game cần DirectX 12 nên thử bản Game Porting Toolkit (có D3DMetal).")
                            .font(.callout)
                            .foregroundStyle(.secondary)
                    }
                    Section("Đã cài") {
                        ForEach(runtimes) { r in EngineRow(runtime: r) }
                    }
                }
                .formStyle(.grouped)
            }
        }
        .navigationTitle("Động cơ")
        .toolbar {
            ToolbarItemGroup(placement: .primaryAction) {
                Button {
                    rescan()
                } label: {
                    Label("Quét lại", systemImage: "arrow.clockwise")
                }
                .help("Quét lại thư mục động cơ")
                Button {
                    addEngine()
                } label: {
                    Label("Thêm động cơ", systemImage: "plus")
                }
                .help("Chọn file wine64 có sẵn trên máy")
            }
        }
    }

    private func rescan() {
        Task { await store.send(["cmd": "rescan"]) }
    }

    private func addEngine() {
        if let p = chooseFile(message: "Chọn file wine64 (hoặc wine) trong thư mục bin của động cơ", insideBundles: true) {
            Task { await store.send(["cmd": "add_runtime", "path": p]) }
        }
    }

    private func openRuntimesFolder() {
        guard let home = store.snap?.home else { return }
        let dir = URL(fileURLWithPath: home).appendingPathComponent("Runtimes")
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        NSWorkspace.shared.open(dir)
    }
}

struct EngineRow: View {
    @Environment(Store.self) private var store
    let runtime: RuntimeItem

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "cpu.fill")
                .imageScale(.large)
                .foregroundStyle(runtime.active ? Color.accentColor : Color.secondary)
                .frame(width: 28)
            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 6) {
                    Text(runtime.name).font(.headline)
                    if runtime.d3dmetal { Badge(text: "D3DMetal", color: .blue) }
                    if runtime.old { Badge(text: "Bản cũ", color: .orange) }
                    if runtime.active { Badge(text: "Mặc định", color: .green) }
                }
                Text(runtime.version ?? "đang đọc phiên bản…")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Text(runtime.path)
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .textSelection(.enabled)
            }
            Spacer()
            if !runtime.active {
                Button("Dùng làm mặc định") {
                    Task { await store.send(["cmd": "use_runtime", "path": runtime.path]) }
                }
            }
        }
        .padding(.vertical, 2)
    }
}
