import SwiftUI

struct LibraryView: View {
    @Environment(Store.self) private var store
    @State private var editing: GameItem?
    @State private var logGame: GameItem?
    @State private var removing: GameItem?
    @State private var steamDialog: GameItem?
    @State private var uninstalling: GameItem?

    private var games: [GameItem] { store.snap?.games ?? [] }

    var body: some View {
        @Bindable var store = store
        Group {
            if store.snap == nil {
                ProgressView("Đang đọc thông tin máy…")
            } else if games.isEmpty {
                ContentUnavailableView {
                    Label("Chưa có game nào", systemImage: "gamecontroller")
                } description: {
                    Text("Thêm game có sẵn trong Steam, tải game mới bằng link Steam, hoặc chọn file .exe. Cần cài Steam cho Windows trước ở mục Môi trường.")
                } actions: {
                    Button("Thêm game…") { store.showAddGame = true }
                        .buttonStyle(.borderedProminent)
                }
            } else {
                List(games) { game in
                    GameRow(game: game,
                            onLog: { logGame = game },
                            onSettings: { editing = game },
                            onRemove: { removing = game },
                            onSteamDialog: { steamDialog = game },
                            onUninstall: { uninstalling = game })
                        .padding(.vertical, 4)
                }
            }
        }
        .navigationTitle("Thư viện")
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    store.showAddGame = true
                } label: {
                    Label("Thêm game", systemImage: "plus")
                }
                .help("Thêm game (⌘N)")
            }
        }
        .sheet(isPresented: $store.showAddGame) { AddGameSheet() }
        .sheet(item: $editing) { game in
            GameSettingsSheet(game: game, onRemove: {
                editing = nil
                removing = game
            }, onSteamDialog: {
                editing = nil
                steamDialog = game
            }, onUninstall: {
                editing = nil
                uninstalling = game
            })
        }
        .sheet(item: $logGame) { LogSheet(game: $0) }
        .sheet(item: $steamDialog) { SteamDialogSheet(game: $0) }
        .alert("Gỡ \(uninstalling?.name ?? "game") khỏi máy?",
               isPresented: Binding(get: { uninstalling != nil }, set: { if !$0 { uninstalling = nil } }),
               presenting: uninstalling) { game in
            Button("Gỡ khỏi máy", role: .destructive) {
                if let appid = game.steamAppid {
                    Task { await store.send(["cmd": "steam_uninstall", "bottle_id": game.bottleId, "appid": appid]) }
                }
            }
            Button("Hủy", role: .cancel) {}
        } message: { game in
            Text("Xóa vĩnh viễn file game\(game.sizeBytes.map { " (\(formatBytes($0)))" } ?? "") khỏi máy để lấy lại dung lượng, và bỏ game khỏi thư viện. File lưu game (save) không bị xóa. Muốn chơi lại phải tải lại toàn bộ. Không hoàn tác được.")
        }
        .alert("Xóa \(removing?.name ?? "game") khỏi thư viện?",
               isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } }),
               presenting: removing) { game in
            Button("Xóa khỏi thư viện", role: .destructive) {
                Task { await store.send(["cmd": "remove_game", "id": game.id]) }
            }
            Button("Hủy", role: .cancel) {}
        } message: { _ in
            Text("File game trên máy vẫn còn nguyên, chỉ bỏ nó khỏi danh sách này.")
        }
    }
}

struct GameRow: View {
    @Environment(Store.self) private var store
    let game: GameItem
    let onLog: () -> Void
    let onSettings: () -> Void
    let onRemove: () -> Void
    var onSteamDialog: () -> Void = {}
    var onUninstall: () -> Void = {}

    var body: some View {
        HStack(spacing: 12) {
            AppTile(name: game.name)
            VStack(alignment: .leading, spacing: 2) {
                Text(game.name).font(.headline)
                if let why = game.steamBlocked {
                    Label("Chưa mở được: \(why).", systemImage: "exclamationmark.triangle.fill")
                        .font(.caption)
                        .foregroundStyle(.orange)
                        .fixedSize(horizontal: false, vertical: true)
                    if game.assist {
                        Button("Xem hộp thoại Steam", action: onSteamDialog)
                            .controlSize(.small)
                    } else {
                        Button("Mở hộp thoại Steam") {
                            Task {
                                await store.send(["cmd": "steam_assist_start", "id": game.id])
                                onSteamDialog()
                            }
                        }
                        .controlSize(.small)
                    }
                } else if game.assist {
                    HStack(spacing: 8) {
                        Label("Cổng xem Steam đang mở (chỉ trong lần chạy này)", systemImage: "eye")
                            .font(.caption)
                            .foregroundStyle(.orange)
                        Button("Xem hộp thoại Steam", action: onSteamDialog)
                            .controlSize(.small)
                    }
                } else if game.exeMissing {
                    Label("Không tìm thấy file game", systemImage: "exclamationmark.triangle.fill")
                        .font(.caption)
                        .foregroundStyle(.orange)
                } else {
                    Text(subtitle).font(.caption).foregroundStyle(.secondary)
                }
                if game.bits == 32 && game.steamBlocked == nil {
                    Label("Game 32-bit: trên Mac chip Apple thường không chạy được (hay sập ngay lúc mở). Cân nhắc CrossOver hoặc chọn game 64-bit.",
                          systemImage: "exclamationmark.triangle.fill")
                        .font(.caption)
                        .foregroundStyle(.orange)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            Spacer()
            status
            Button {
                onLog()
            } label: {
                Image(systemName: "doc.text")
            }
            .help("Xem log")
            .accessibilityLabel("Xem log")
            .disabled(!game.hasLog)
            Button {
                onSettings()
            } label: {
                Image(systemName: "gearshape")
            }
            .help("Cài đặt game")
            .accessibilityLabel("Cài đặt game")
            playButton
        }
        .contextMenu {
            Button("Cài đặt…", systemImage: "slider.horizontal.3", action: onSettings)
            if game.isSteam {
                Button("Xem hộp thoại Steam…", systemImage: "eye", action: onSteamDialog)
                if game.steamAppid != nil && !game.isRunning {
                    Button("Gỡ game khỏi máy…", systemImage: "externaldrive.badge.minus", role: .destructive, action: onUninstall)
                }
            }
            Divider()
            Button("Xóa khỏi thư viện…", systemImage: "trash", role: .destructive, action: onRemove)
        }
    }

    private var subtitle: String {
        var parts: [String] = []
        if let b = game.bottleName { parts.append(b) } else { parts.append("Môi trường đã bị xóa") }
        if game.isSteam { parts.append("Steam") }
        if let size = game.sizeBytes, size > 0 { parts.append(formatBytes(size)) }
        return parts.joined(separator: " · ")
    }

    @ViewBuilder
    private var status: some View {
        if let secs = game.runningSecs {
            HStack(spacing: 5) {
                Circle().fill(.green).frame(width: 7, height: 7)
                Text("Đang chạy · \(formatElapsed(secs))")
                    .font(.caption)
                    .monospacedDigit()
                    .foregroundStyle(.secondary)
            }
        } else if game.uninstalling {
            HStack(spacing: 6) {
                ProgressView().controlSize(.small)
                Text("Đang gỡ…").font(.caption).foregroundStyle(.secondary)
            }
        } else if game.stopping {
            HStack(spacing: 6) {
                ProgressView().controlSize(.small)
                Text("Đang dừng…").font(.caption).foregroundStyle(.secondary)
            }
        } else if game.waiting {
            HStack(spacing: 6) {
                ProgressView().controlSize(.small)
                Text("Đang chuẩn bị…").font(.caption).foregroundStyle(.secondary)
            }
        }
    }

    @ViewBuilder
    private var playButton: some View {
        if game.uninstalling {
            Button("Chơi") {}
                .buttonStyle(.bordered)
                .frame(minWidth: 64)
                .disabled(true)
        } else if game.stopping {
            Button("Dừng") {}
                .buttonStyle(.bordered)
                .frame(minWidth: 64)
                .disabled(true)
        } else if game.isRunning || game.waiting {
            Button("Dừng") {
                Task { await store.send(["cmd": "stop", "id": game.id]) }
            }
            .buttonStyle(.bordered)
            .tint(.red)
            .frame(minWidth: 64)
            // A stop right after start would kill Wine while it is still setting up.
            .disabled((game.runningSecs ?? 99) < 2)
        } else {
            Button("Chơi") {
                Task { await store.send(["cmd": "play", "id": game.id]) }
            }
            .buttonStyle(.borderedProminent)
            .frame(minWidth: 64)
            .disabled(game.exeMissing)
        }
    }
}

// MARK: - Add game

struct AddGameSheet: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var mode: Mode = .steam

    enum Mode: String, CaseIterable, Identifiable {
        case steam = "Từ Steam"
        case exe = "Từ file .exe"
        var id: String { rawValue }
    }

    private var hasSteam: Bool { store.snap?.bottles.contains(where: \.steamInstalled) ?? false }

    var body: some View {
        VStack(spacing: 0) {
            Picker("Cách thêm", selection: $mode) {
                ForEach(Mode.allCases) { Text($0.rawValue).tag($0) }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .padding(.horizontal, 20)
            .padding(.top, 16)
            switch mode {
            case .steam: SteamAddView(onClose: { dismiss() })
            case .exe: ExeAddView(onClose: { dismiss() })
            }
        }
        .frame(width: 560)
        .onAppear { mode = hasSteam ? .steam : .exe }
        .onDisappear { Task { await store.send(["cmd": "steam_lookup_clear"]) } }
    }
}

/// Games that Steam (inside a bottle) already holds, plus a guided way to download new ones.
struct SteamAddView: View {
    @Environment(Store.self) private var store
    let onClose: () -> Void
    @State private var input = ""
    @State private var bottleId = ""
    @State private var searched = false
    @State private var account = ""
    @State private var uninstallTarget: SteamGameItem?

    private var snap: Snapshot? { store.snap }
    private var steamBottles: [BottleItem] { snap?.bottles.filter(\.steamInstalled) ?? [] }
    private var ready: [SteamGameItem] { snap?.steamGames.filter { $0.installed && !$0.inLibrary } ?? [] }
    private var downloading: [SteamGameItem] { snap?.steamGames.filter { !$0.installed } ?? [] }

    var body: some View {
        VStack(spacing: 0) {
            Form {
                if steamBottles.isEmpty {
                    Section {
                        Label("Chưa có môi trường nào cài Steam. Vào mục Môi trường, chọn Cài Steam.", systemImage: "info.circle")
                            .foregroundStyle(.secondary)
                    }
                } else {
                    Section("Game đã có trong Steam của máy") {
                        if ready.isEmpty {
                            Text("Không có game mới. Game đã thêm vào thư viện thì không hiện ở đây.")
                                .font(.callout).foregroundStyle(.secondary)
                        }
                        ForEach(ready) { g in
                            HStack {
                                AppTile(name: g.name, size: 30)
                                VStack(alignment: .leading, spacing: 0) {
                                    HStack(spacing: 6) {
                                        Text(g.name)
                                        if g.bits == 32 { Badge(text: "32-bit", color: .orange) }
                                    }
                                    if g.bits == 32 {
                                        Text("Thường không chạy được trên Mac chip Apple")
                                            .font(.caption).foregroundStyle(.orange)
                                    } else {
                                        Text(g.bottleName).font(.caption).foregroundStyle(.secondary)
                                    }
                                }
                                Spacer()
                                if g.uninstalling {
                                    ProgressView().controlSize(.small)
                                    Text("Đang gỡ…").font(.caption).foregroundStyle(.secondary)
                                } else {
                                    Text(formatBytes(g.sizeBytes)).font(.caption).foregroundStyle(.secondary)
                                    Button("Gỡ…", role: .destructive) { uninstallTarget = g }
                                    Button("Thêm") {
                                        Task {
                                            await store.send(["cmd": "add_steam_game", "bottle_id": g.bottleId, "appid": g.appid])
                                        }
                                    }
                                }
                            }
                        }
                        ForEach(downloading) { g in
                            HStack {
                                AppTile(name: g.name, size: 30)
                                Text(g.name)
                                Spacer()
                                Text("Chưa tải xong").font(.caption).foregroundStyle(.secondary)
                            }
                        }
                    }
                    if let d = snap?.download {
                        Section("Đang tải game") { DownloadPanel(job: d) }
                    } else {
                        Section {
                            HStack {
                                TextField("Dán link trang game trên Steam", text: $input)
                                    .onSubmit(search)
                                Button("Tìm", action: search)
                                    .disabled(input.trimmingCharacters(in: .whitespaces).isEmpty)
                            }
                            if let l = snap?.lookup, searched {
                                lookupResult(l)
                            }
                        } header: {
                            Text("Cài game mới")
                        } footer: {
                            Text("Mở trang game trên steampowered.com, sao chép đường dẫn rồi dán vào đây. M2WGame tải game bằng SteamCMD ngay trong cửa sổ này. Mật khẩu chỉ được gửi cho SteamCMD, không được lưu lại.")
                        }
                    }
                }
            }
            .formStyle(.grouped)
            .frame(height: 430)
            .alert("Gỡ \(uninstallTarget?.name ?? "game") khỏi máy?",
                   isPresented: Binding(get: { uninstallTarget != nil }, set: { if !$0 { uninstallTarget = nil } }),
                   presenting: uninstallTarget) { g in
                Button("Gỡ khỏi máy", role: .destructive) {
                    Task { await store.send(["cmd": "steam_uninstall", "bottle_id": g.bottleId, "appid": g.appid]) }
                }
                Button("Hủy", role: .cancel) {}
            } message: { g in
                Text("Xóa vĩnh viễn file game (\(formatBytes(g.sizeBytes))) khỏi máy để lấy lại dung lượng. File lưu game (save) không bị xóa. Muốn chơi lại phải tải lại toàn bộ. Không hoàn tác được.")
            }
            HStack {
                Button("Quét lại") { Task { await store.refresh() } }
                Spacer()
                Button("Xong", action: onClose)
                    .keyboardShortcut(.defaultAction)
                    .buttonStyle(.borderedProminent)
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 14)
        }
        .onAppear {
            // Windows Steam crashes on the old Wine 7 engine, so prefer an environment with a newer one.
            if bottleId.isEmpty {
                bottleId = (steamBottles.first(where: { !$0.effectiveOld }) ?? steamBottles.first)?.id ?? ""
            }
            if account.isEmpty { account = snap?.steamAccount ?? "" }
        }
    }

    @ViewBuilder
    private func lookupResult(_ l: LookupItem) -> some View {
        if l.busy {
            HStack(spacing: 6) { ProgressView().controlSize(.small); Text("Đang tìm…") }
        } else if let err = l.error {
            Label(err, systemImage: "exclamationmark.triangle.fill").foregroundStyle(.orange)
        } else if let name = l.name {
            HStack {
                AppTile(name: name, size: 30)
                VStack(alignment: .leading, spacing: 0) {
                    Text(name).font(.headline)
                    Text("Mã \(l.appid)").font(.caption).foregroundStyle(.secondary)
                }
            }
            if !l.windows {
                Label("Game này không có bản Windows nên không chạy được ở đây.", systemImage: "xmark.octagon.fill")
                    .foregroundStyle(.red)
            } else {
                if steamBottles.count > 1 {
                    Picker("Cài vào môi trường", selection: $bottleId) {
                        ForEach(steamBottles) { Text($0.name).tag($0.id) }
                    }
                }
                if snap?.steamcmdReady == false {
                    Label("Chưa có SteamCMD trong thư mục dữ liệu của M2WGame nên chưa tải được.", systemImage: "exclamationmark.triangle.fill")
                        .foregroundStyle(.orange)
                }
                TextField("Tài khoản Steam (tên đăng nhập)", text: $account)
                HStack {
                    Button("Dùng Terminal thay thế") {
                        Task {
                            await store.send(["cmd": "steam_install", "bottle_id": bottleId, "appid": l.appid])
                        }
                    }
                    .buttonStyle(.link)
                    .disabled(bottleId.isEmpty || snap?.steamcmdReady == false)
                    Spacer()
                    Button("Tải về") {
                        Task {
                            await store.send(["cmd": "steam_dl_start", "bottle_id": bottleId,
                                              "appid": l.appid, "account": account])
                        }
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(bottleId.isEmpty || account.trimmingCharacters(in: .whitespaces).isEmpty
                              || snap?.steamcmdReady == false)
                }
            }
        }
    }

    private func search() {
        let t = input.trimmingCharacters(in: .whitespaces)
        guard !t.isEmpty else { return }
        searched = true
        Task { await store.send(["cmd": "steam_lookup", "input": t]) }
    }
}

/// Progress and questions of a running SteamCMD download.
struct DownloadPanel: View {
    @Environment(Store.self) private var store
    let job: DownloadItem
    @State private var secret = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(job.name ?? "Game \(job.appid)").font(.headline)
            switch job.phase {
            case "need_password":
                prompt(field: SecureField("Mật khẩu Steam", text: $secret), button: "Gửi")
                Text(job.message).font(.caption).foregroundStyle(.secondary)
            case "need_code":
                prompt(field: TextField("Mã Steam Guard", text: $secret), button: "Gửi")
                Text(job.message).font(.caption).foregroundStyle(.secondary)
            case "waiting_mobile":
                HStack(spacing: 8) { ProgressView().controlSize(.small); Text(job.message) }
            case "downloading":
                ProgressView(value: min(max(job.percent, 0), 100), total: 100)
                Text(job.message).font(.caption).foregroundStyle(.secondary)
            case "done":
                Label(job.message, systemImage: "checkmark.circle.fill").foregroundStyle(.green)
            case "failed":
                Label(job.message, systemImage: "exclamationmark.triangle.fill").foregroundStyle(.orange)
            default:
                HStack(spacing: 8) { ProgressView().controlSize(.small); Text(job.message) }
            }
            if !job.tail.isEmpty && job.phase != "done" {
                DisclosureGroup("Chi tiết") {
                    Text(job.tail.joined(separator: "\n"))
                        .font(.system(size: 10, design: .monospaced))
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .font(.caption)
            }
            HStack {
                Spacer()
                if job.finished {
                    Button(job.phase == "done" ? "Xong" : "Đóng") {
                        Task { await store.send(["cmd": "steam_dl_clear"]) }
                    }
                    .buttonStyle(.borderedProminent)
                } else {
                    Button("Hủy tải", role: .destructive) {
                        Task { await store.send(["cmd": "steam_dl_cancel"]) }
                    }
                }
            }
        }
        .padding(.vertical, 4)
    }

    private func prompt<F: View>(field: F, button: String) -> some View {
        HStack {
            field.onSubmit(submit)
            Button(button, action: submit)
                .buttonStyle(.borderedProminent)
                .disabled(secret.isEmpty)
        }
    }

    private func submit() {
        let text = secret
        guard !text.isEmpty else { return }
        secret = ""
        Task { await store.send(["cmd": "steam_dl_input", "text": text]) }
    }
}

struct ExeAddView: View {
    @Environment(Store.self) private var store
    let onClose: () -> Void
    @State private var name = ""
    @State private var exe = ""
    @State private var bottleId = ""
    @State private var args = ""
    @FocusState private var nameFocused: Bool

    private var bottles: [BottleItem] { store.snap?.bottles ?? [] }

    var body: some View {
        VStack(spacing: 0) {
            Form {
                Section {
                    TextField("Tên game", text: $name)
                        .focused($nameFocused)
                    LabeledContent("File chạy (.exe)") {
                        HStack {
                            Text(exe.isEmpty ? "Chưa chọn" : exe)
                                .lineLimit(1)
                                .truncationMode(.middle)
                                .foregroundStyle(exe.isEmpty ? .tertiary : .secondary)
                            Button("Chọn…") {
                                if let p = chooseFile(message: "Chọn file .exe của game", extensions: ["exe"]) {
                                    exe = p
                                    if name.isEmpty {
                                        name = URL(fileURLWithPath: p).deletingPathExtension().lastPathComponent
                                    }
                                }
                            }
                        }
                    }
                    Picker("Môi trường", selection: $bottleId) {
                        ForEach(bottles) { Text($0.name).tag($0.id) }
                    }
                    TextField("Tham số thêm (không bắt buộc)", text: $args)
                } footer: {
                    Text("Dùng cách này cho game không qua Steam (bản cài sẵn, GOG, bản tải riêng). Game Steam nên thêm ở tab Từ Steam.")
                }
            }
            .formStyle(.grouped)
            SheetButtons(actionTitle: "Thêm",
                         actionEnabled: !name.trimmingCharacters(in: .whitespaces).isEmpty && !exe.isEmpty,
                         onCancel: onClose) {
                Task {
                    let ok = await store.send(["cmd": "add_game", "name": name, "exe": exe,
                                               "bottle_id": bottleId, "args": args])
                    if ok { onClose() }
                }
            }
        }
        .onAppear {
            if bottleId.isEmpty { bottleId = bottles.first?.id ?? "" }
            nameFocused = true
        }
    }
}

// MARK: - Game settings

struct GameSettingsSheet: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var draft: GamePayload
    @State private var preview: String?
    private let original: GameItem
    private let onRemove: () -> Void
    private let onSteamDialog: () -> Void
    private let onUninstall: () -> Void

    init(game: GameItem, onRemove: @escaping () -> Void, onSteamDialog: @escaping () -> Void, onUninstall: @escaping () -> Void) {
        original = game
        self.onRemove = onRemove
        self.onSteamDialog = onSteamDialog
        self.onUninstall = onUninstall
        _draft = State(initialValue: GamePayload(game))
    }

    private var bottles: [BottleItem] { store.snap?.bottles ?? [] }

    var body: some View {
        VStack(spacing: 0) {
            Text("Cài đặt: \(original.name)")
                .font(.headline)
                .padding(.top, 16)
            if store.snap?.games.first(where: { $0.id == original.id })?.isRunning == true {
                Label("Game đang chạy: thay đổi chỉ áp dụng khi bạn Dừng rồi Chơi lại.",
                      systemImage: "info.circle.fill")
                    .font(.caption)
                    .foregroundStyle(.orange)
                    .padding(.top, 4)
            }
            Form {
                Section("Thông tin") {
                    TextField("Tên game", text: $draft.name)
                    Picker("Môi trường", selection: $draft.bottleId) {
                        ForEach(bottles) { Text($0.name).tag($0.id) }
                    }
                    if !original.isSteam {
                        LabeledContent("File game") {
                            Text(draft.exe)
                                .lineLimit(2)
                                .truncationMode(.middle)
                                .textSelection(.enabled)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
                Section {
                    Toggle(isOn: $draft.settings.metalHud) {
                        option("Hiện số khung hình mỗi giây (FPS)",
                               "Hiện một bảng nhỏ ở góc màn hình khi chơi, để biết game có mượt không. Cần Dừng rồi Chơi lại mới có hiệu lực.")
                    }
                } header: {
                    Text("Hiển thị")
                }
                if original.isSteam {
                    Section {
                        Button("Mở hộp thoại Steam…") {
                            Task {
                                await store.send(["cmd": "steam_assist_start", "id": original.id])
                                onSteamDialog()
                            }
                        }
                        if original.steamAppid != nil {
                            Button("Gỡ game khỏi máy…\(original.sizeBytes.map { " (giải phóng \(formatBytes($0)))" } ?? "")", role: .destructive, action: onUninstall)
                        }
                    } header: {
                        Text("Steam")
                    } footer: {
                        Text("Dùng \"Mở hộp thoại Steam\" khi game không mở được vì Steam đang hỏi điều gì đó (điều khoản, đồng bộ đám mây): M2WGame hiện câu hỏi đó để bạn tự trả lời. Chỉ có hiệu lực trong lần chạy này.")
                    }
                }
                Section {
                    DisclosureGroup("Nâng cao (chỉ dùng khi game không chạy được)") {
                        Text("Bình thường bạn không cần đổi gì ở đây.")
                            .font(.caption).foregroundStyle(.secondary)
                        Toggle(isOn: $draft.settings.msync) {
                            option("Chạy mượt hơn (nên để bật)", "Chỉ tắt nếu game bị lỗi lạ.")
                        }
                        Toggle(isOn: $draft.settings.esync) {
                            option("Chế độ tương thích cũ", "Chỉ thử bật khi game lỗi dù mục trên đã bật.")
                        }
                        Toggle(isOn: $draft.settings.advertiseAvx) {
                            option("Giả lập tính năng AVX của máy (nên để bật)", "Một số game cần mới mở được. Cần macOS 15 trở lên.")
                        }
                        Toggle(isOn: $draft.settings.debugLog) {
                            option("Ghi nhật ký chi tiết", "Giúp tìm nguyên nhân khi game không mở. Nhật ký dài hơn, đọc bằng nút Log.")
                        }
                        if original.isSteam {
                            Text("Cách mở game này do M2WGame tự quản lý (qua Steam), bạn không cần chỉnh.")
                                .font(.caption).foregroundStyle(.secondary)
                        } else {
                            VStack(alignment: .leading, spacing: 2) {
                                TextField("Tham số thêm", text: $draft.args)
                                Text("Chữ gửi thêm cho game khi mở, ví dụ -windowed. Để trống nếu không biết.")
                                    .font(.caption).foregroundStyle(.secondary)
                            }
                        }
                        VStack(alignment: .leading, spacing: 4) {
                            Text("Biến môi trường (dành cho người rành kỹ thuật)")
                            TextEditor(text: $draft.settings.extraEnv)
                                .font(.system(.body, design: .monospaced))
                                .frame(height: 64)
                            Text("Mỗi dòng một biến, dạng TÊN=giá trị.")
                                .font(.caption).foregroundStyle(.secondary)
                        }
                        DisclosureGroup("Lệnh sẽ chạy (theo cài đặt đã lưu)") {
                            Text(preview ?? "Đang đọc…")
                                .font(.system(.caption, design: .monospaced))
                                .textSelection(.enabled)
                                .frame(maxWidth: .infinity, alignment: .leading)
                        }
                    }
                }
            }
            .formStyle(.grouped)
            HStack {
                Button("Xóa khỏi thư viện…", role: .destructive, action: onRemove)
                    .padding(.leading, 20)
                SheetButtons(actionTitle: "Lưu",
                             actionEnabled: !draft.name.trimmingCharacters(in: .whitespaces).isEmpty,
                             onCancel: { dismiss() }) {
                    Task {
                        let ok = await store.send(["cmd": "update_game", "game": Bridge.object(draft)])
                        if ok { dismiss() }
                    }
                }
            }
        }
        .frame(width: 560, height: 640)
        .task { preview = await store.fetchPreview(id: original.id) }
    }

    private func option(_ title: String, _ detail: String) -> some View {
        VStack(alignment: .leading, spacing: 1) {
            Text(title)
            Text(detail).font(.caption).foregroundStyle(.secondary)
        }
    }
}

// MARK: - Log

struct LogSheet: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss
    let game: GameItem
    @State private var reply: LogReply?
    @State private var loading = true

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text("Log: \(game.name)").font(.headline)
                Spacer()
                Button {
                    Task { await load() }
                } label: {
                    Label("Tải lại", systemImage: "arrow.clockwise")
                }
                if let path = reply?.path {
                    Button {
                        reveal(path)
                    } label: {
                        Label("Hiện trong Finder", systemImage: "folder")
                    }
                }
            }
            .padding(16)
            Divider()
            if let hints = reply?.hints, !hints.isEmpty {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Gợi ý").font(.subheadline.weight(.semibold))
                    ForEach(hints) { h in
                        HStack(alignment: .top, spacing: 8) {
                            LevelIcon(level: h.level)
                            Text(h.text).font(.callout).fixedSize(horizontal: false, vertical: true)
                        }
                    }
                }
                .padding(16)
                Divider()
            }
            if loading && reply == nil {
                ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if let err = reply?.error {
                Text(err).foregroundStyle(.secondary).padding(16)
                Spacer()
            } else {
                ScrollView {
                    Text(reply?.text?.isEmpty == false ? reply!.text! : "(log trống)")
                        .font(.system(size: 11, design: .monospaced))
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(12)
                }
                .background(Color(nsColor: .textBackgroundColor))
            }
            Divider()
            HStack {
                Spacer()
                Button("Đóng") { dismiss() }
                    .keyboardShortcut(.defaultAction)
                    .buttonStyle(.borderedProminent)
            }
            .padding(12)
        }
        .frame(width: 720, height: 560)
        .task { await load() }
    }

    private func load() async {
        loading = true
        reply = await store.fetchLog(id: game.id)
        loading = false
    }
}
