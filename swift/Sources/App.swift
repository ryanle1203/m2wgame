import SwiftUI

@main
struct M2WGameApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @State private var store = Store()

    var body: some Scene {
        Window("M2WGame", id: "main") {
            ContentView()
                .environment(store)
                .environment(\.locale, Locale(identifier: store.languageCode))
                .id(store.languageCode)
                .frame(minWidth: 860, minHeight: 560)
                .task {
                    AppDelegate.store = store
                    store.start()
                }
        }
        .defaultSize(width: 1080, height: 720)
        .commands {
            CommandGroup(replacing: .newItem) {
                Button(L("Thêm game…")) {
                    store.section = .library
                    store.showAddGame = true
                }
                .keyboardShortcut("n")
            }
            CommandMenu(L("Đi tới")) {
                ForEach(Array(Store.Section.allCases.enumerated()), id: \.element) { index, section in
                    Button(section.title) { store.section = section }
                        .keyboardShortcut(KeyEquivalent(Character("\(index + 1)")))
                }
            }
            CommandMenu(L("Ngôn ngữ")) {
                Picker(L("Ngôn ngữ"), selection: Binding(get: { store.languageChoice },
                                                         set: { store.languageChoice = $0 })) {
                    ForEach(LanguageChoice.allCases) { Text($0.title).tag($0) }
                }
                .pickerStyle(.inline)
            }
        }
    }
}

struct ContentView: View {
    @Environment(Store.self) private var store

    var body: some View {
        @Bindable var store = store
        NavigationSplitView {
            Sidebar()
        } detail: {
            Group {
                switch store.section {
                case .library: LibraryView()
                case .environments: EnvironmentsView()
                case .engines: EnginesView()
                case .system: SystemView()
                }
            }
            .overlay(alignment: .bottom) {
                if let toast = store.toast {
                    ToastView(notice: toast)
                        .padding(16)
                        .transition(.move(edge: .bottom).combined(with: .opacity))
                }
            }
            .animation(.snappy, value: store.toast)
        }
    }
}

struct Sidebar: View {
    @Environment(Store.self) private var store

    var body: some View {
        @Bindable var store = store
        List(selection: $store.section) {
            ForEach(Store.Section.allCases) { section in
                Label(section.title, systemImage: section.symbol)
                    .tag(section)
            }
        }
        .navigationSplitViewColumnWidth(min: 180, ideal: 210, max: 280)
        .safeAreaInset(edge: .bottom) {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 6) {
                    Image(systemName: "globe").foregroundStyle(.secondary)
                    Picker("Ngôn ngữ", selection: $store.languageChoice) {
                        ForEach(LanguageChoice.allCases) { Text($0.title).tag($0) }
                    }
                    .labelsHidden()
                    .pickerStyle(.menu)
                    .controlSize(.small)
                    .fixedSize()
                }
                if let d = store.snap?.download, !d.finished {
                    HStack(spacing: 6) {
                        ProgressView().controlSize(.small)
                        Text(downloadLabel(d)).font(.caption).foregroundStyle(.secondary)
                    }
                }
                if let n = store.snap?.runningCount, n > 0 {
                    HStack(spacing: 6) {
                        Circle().fill(.green).frame(width: 8, height: 8)
                        Text("\(n) game đang chạy")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(12)
        }
    }
}

private func downloadLabel(_ d: DownloadItem) -> String {
    switch d.phase {
    case "need_password": return L("Tải game: cần mật khẩu")
    case "need_code": return L("Tải game: cần mã Steam Guard")
    case "waiting_mobile": return L("Tải game: chờ duyệt trên điện thoại")
    case "downloading": return L("Đang tải game %lld%%", Int(d.percent))
    default: return L("Đang chuẩn bị tải game…")
    }
}

struct ToastView: View {
    @Environment(Store.self) private var store
    let notice: Notice

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: notice.error ? "exclamationmark.triangle.fill" : "info.circle.fill")
                .foregroundStyle(notice.error ? Color.orange : Color.accentColor)
                .imageScale(.large)
            Text(notice.text)
                .font(.callout)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
            Button {
                store.dismissToast()
            } label: {
                Image(systemName: "xmark.circle.fill").foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
            .help("Đóng")
        }
        .padding(12)
        .frame(maxWidth: 560)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(.separator))
        .shadow(color: .black.opacity(0.15), radius: 12, y: 4)
    }
}
