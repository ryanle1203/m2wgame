import AppKit
import Foundation
import Observation

/// The only place that talks to the Rust core.
enum Bridge {
    static let decoder: JSONDecoder = {
        let d = JSONDecoder()
        d.keyDecodingStrategy = .convertFromSnakeCase
        return d
    }()

    static let encoder: JSONEncoder = {
        let e = JSONEncoder()
        e.keyEncodingStrategy = .convertToSnakeCase
        return e
    }()

    /// Send one command, get the raw JSON answer. Runs off the main thread.
    static func call(_ command: [String: Any]) async -> Data? {
        guard let body = try? JSONSerialization.data(withJSONObject: command),
              let text = String(data: body, encoding: .utf8) else { return nil }
        return await Task.detached(priority: .userInitiated) { () -> Data? in
            guard let out = wb_request(text) else { return nil }
            defer { wb_free(out) }
            return Data(String(cString: out).utf8)
        }.value
    }

    /// Turn an Encodable value into a JSON object usable inside a command.
    static func object<T: Encodable>(_ value: T) -> Any {
        guard let data = try? encoder.encode(value),
              let obj = try? JSONSerialization.jsonObject(with: data) else { return [:] }
        return obj
    }
}

@MainActor
@Observable
final class Store {
    var snap: Snapshot?
    var toast: Notice?
    var showAddGame = false
    var section: Section = .library

    /// Developer hooks, used to take documentation screenshots without clicking:
    /// `M2WGAME_START_SECTION` = library | environments | engines | system,
    /// `M2WGAME_OPEN` = add_game | settings | check_net.
    let startAction: String? = ProcessInfo.processInfo.environment["M2WGAME_OPEN"]

    /// The interface language the user chose (saved), and the one actually in use ("en" / "vi").
    var languageChoice: LanguageChoice = LanguageChoice.loaded() {
        didSet {
            languageChoice.save()
            applyLanguage()
        }
    }
    var languageCode = "vi"

    private func applyLanguage() {
        languageCode = languageChoice.code
        activeLanguage = languageCode
        let code = languageCode
        Task { _ = await Bridge.call(["cmd": "set_language", "lang": code]) }
    }

    init() {
        let env = ProcessInfo.processInfo.environment
        languageCode = languageChoice.code
        activeLanguage = languageCode
        if let s = env["M2WGAME_START_SECTION"], let sec = Section(rawValue: s) { section = sec }
        if env["M2WGAME_OPEN"] == "add_game" { showAddGame = true }
    }

    private var lastNoticeId = 0
    private var localNoticeId = 0
    private var started = false

    enum Section: String, CaseIterable, Identifiable {
        case library, environments, engines, system
        var id: String { rawValue }

        var title: String {
            switch self {
            case .library: L("Thư viện")
            case .environments: L("Môi trường")
            case .engines: L("Động cơ")
            case .system: L("Hệ thống")
            }
        }

        var symbol: String {
            switch self {
            case .library: "gamecontroller.fill"
            case .environments: "shippingbox.fill"
            case .engines: "cpu.fill"
            case .system: "checkmark.shield.fill"
            }
        }
    }

    // MARK: polling

    func start() {
        guard !started else { return }
        started = true
        // Tell the core the language before anything else, so its texts come out right.
        let code = languageCode
        if startAction == "check_net" { Task { _ = await send(["cmd": "check_net"]) } }
        Task {
            _ = await Bridge.call(["cmd": "set_language", "lang": code])
            while true {
                await refresh()
                try? await Task.sleep(for: .milliseconds(snap?.runningCount ?? 0 > 0 ? 1000 : 1500))
            }
        }
    }

    func refresh() async {
        guard let data = await Bridge.call(["cmd": "state"]),
              let s = try? Bridge.decoder.decode(Snapshot.self, from: data) else { return }
        if s != snap { snap = s }
        if let n = s.notice, n.id != lastNoticeId {
            lastNoticeId = n.id
            show(n)
        }
    }

    // MARK: commands

    /// Run a command; failures become a message for the user. Returns true when it worked.
    @discardableResult
    func send(_ command: [String: Any]) async -> Bool {
        guard let data = await Bridge.call(command),
              let reply = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            showLocal(L("Không nói chuyện được với lõi của ứng dụng."), error: true)
            return false
        }
        let ok = reply["ok"] as? Bool ?? false
        await refresh()
        if !ok {
            // The core usually posts the same message itself; only add ours if it did not.
            let text = reply["error"] as? String ?? L("Có lỗi xảy ra.")
            if toast?.text != text { showLocal(text, error: true) }
        }
        return ok
    }

    func fetchLog(id: String) async -> LogReply? {
        guard let data = await Bridge.call(["cmd": "log", "id": id]) else { return nil }
        return try? Bridge.decoder.decode(LogReply.self, from: data)
    }

    func fetchPreview(id: String) async -> String? {
        guard let data = await Bridge.call(["cmd": "preview", "id": id]),
              let reply = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        return reply["command"] as? String
    }

    // MARK: notices

    private func show(_ n: Notice) {
        toast = n
        if !n.error {
            Task {
                try? await Task.sleep(for: .seconds(9))
                if toast == n { dismissToast() }
            }
        }
    }

    private func showLocal(_ text: String, error: Bool) {
        localNoticeId -= 1
        show(Notice(id: localNoticeId, text: text, error: error))
    }

    func dismissToast() {
        toast = nil
        Task { _ = await Bridge.call(["cmd": "dismiss_notice"]) }
    }
}

// MARK: - Quitting

/// Quitting while games run asks the user what should happen to them.
final class AppDelegate: NSObject, NSApplicationDelegate {
    @MainActor static var store: Store?

    /// Closing the window quits the app, unless a game is running: then the app stays in the Dock
    /// (click it to bring the window back) so the game can still be stopped from M2WGame.
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        guard let games = MainActor.assumeIsolated({ Self.store?.snap?.games }) else { return true }
        return !games.contains { $0.isRunning || $0.stopping }
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard let store = Self.store, let games = store.snap?.games else { return .terminateNow }
        let running = games.filter { $0.isRunning || $0.stopping }
        if running.isEmpty { return .terminateNow }

        let alert = NSAlert()
        alert.messageText = L("Đang có game đang chạy")
        alert.informativeText = L("%@. Nếu thoát mà không dừng, game vẫn chạy nhưng M2WGame sẽ không còn điều khiển được nó.", running.map(\.name).joined(separator: ", "))
        alert.addButton(withTitle: L("Dừng game rồi thoát"))
        alert.addButton(withTitle: L("Thoát, để game chạy tiếp"))
        alert.addButton(withTitle: L("Hủy"))
        switch alert.runModal() {
        case .alertFirstButtonReturn:
            Task { @MainActor in
                for g in running where g.isRunning { await store.send(["cmd": "stop", "id": g.id]) }
                // wait (up to ~60 s: Steam is asked to close gently) until nothing is running or stopping
                for _ in 0..<120 {
                    await store.refresh()
                    if store.snap?.games.contains(where: { $0.isRunning || $0.stopping }) != true { break }
                    try? await Task.sleep(for: .milliseconds(500))
                }
                NSApp.reply(toApplicationShouldTerminate: true)
            }
            return .terminateLater
        case .alertSecondButtonReturn:
            return .terminateNow
        default:
            return .terminateCancel
        }
    }
}
