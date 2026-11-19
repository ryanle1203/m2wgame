import Foundation

// Everything the Rust core reports in one snapshot ({"cmd":"state"}).
// Keys arrive in snake_case and are converted by `Bridge.decoder`.

struct GameSettings: Codable, Equatable {
    var msync = true
    var esync = false
    var metalHud = false
    var advertiseAvx = true
    var debugLog = false
    var extraEnv = ""
}

struct GameItem: Decodable, Identifiable, Equatable {
    var id: String
    var name: String
    var exe: String
    var bottleId: String
    var bottleName: String?
    var args: String
    var settings: GameSettings
    var exeMissing: Bool
    var isSteam: Bool
    var runningSecs: Int?
    var waiting: Bool
    var stopping: Bool
    var steamBlocked: String?
    var assist: Bool
    var bits: Int?
    var steamAppid: String?
    var sizeBytes: Int64?
    var uninstalling: Bool
    var hasLog: Bool

    var isRunning: Bool { runningSecs != nil }
}

/// What the settings sheet sends back with `update_game`.
struct GamePayload: Encodable, Equatable {
    var id: String
    var name: String
    var exe: String
    var bottleId: String
    var args: String
    var settings: GameSettings

    init(_ g: GameItem) {
        id = g.id; name = g.name; exe = g.exe
        bottleId = g.bottleId; args = g.args; settings = g.settings
    }
}

struct BottleItem: Decodable, Identifiable, Equatable {
    var id: String
    var name: String
    var path: String
    var initialized: Bool
    var busy: Bool
    var runtime: String?
    var effectiveRuntime: String?
    var effectiveOld: Bool
    var steamInstalled: Bool
    var steamInLibrary: Bool
    var installingSteam: Bool
    var gameCount: Int
}

struct RuntimeItem: Decodable, Identifiable, Equatable {
    var path: String
    var name: String
    var d3dmetal: Bool
    var version: String?
    var old: Bool
    var active: Bool

    var id: String { path }
}

struct CheckItem: Decodable, Identifiable, Equatable {
    var label: String
    var detail: String
    var level: Level

    var id: String { label }
}

enum Level: String, Decodable {
    case ok, warn, bad
}

struct SystemStatus: Decodable, Equatable {
    var ready: Bool
    var checks: [CheckItem]
}

struct NetResult: Decodable, Identifiable, Equatable {
    var id: String
    var label: String
    var why: String
    var status: String
    var level: Level
    var detail: String
}

struct AdviceItem: Decodable, Identifiable, Equatable {
    var level: Level
    var text: String

    var id: String { text }
}

struct NetInfo: Decodable, Equatable {
    var busy: Bool
    var results: [NetResult]?
    var advice: [AdviceItem]?
}

struct Notice: Decodable, Equatable {
    var id: Int
    var text: String
    var error: Bool
}

struct SteamGameItem: Decodable, Identifiable, Equatable {
    var bottleId: String
    var bottleName: String
    var appid: String
    var name: String
    var installed: Bool
    var bits: Int?
    var sizeBytes: Int64
    var uninstalling: Bool
    var inLibrary: Bool

    var id: String { bottleId + "/" + appid }
}

struct LookupItem: Decodable, Equatable {
    var appid: String
    var busy: Bool
    var name: String?
    var windows: Bool
    var error: String?
}

struct DownloadItem: Decodable, Equatable {
    var appid: String
    var bottleId: String
    var name: String?
    var phase: String
    var percent: Double
    var message: String
    var tail: [String]

    var finished: Bool { phase == "done" || phase == "failed" }
}

struct Snapshot: Decodable, Equatable {
    var games: [GameItem]
    var download: DownloadItem?
    var steamAccount: String
    var steamGames: [SteamGameItem]
    var lookup: LookupItem?
    var steamcmdReady: Bool
    var bottles: [BottleItem]
    var runtimes: [RuntimeItem]
    var defaultRuntime: String?
    var system: SystemStatus
    var net: NetInfo
    var runningCount: Int
    var notice: Notice?
    var home: String
    var logsDir: String
}

struct HintItem: Decodable, Identifiable, Equatable {
    var level: Level
    var text: String

    var id: String { text }
}

struct LogReply: Decodable {
    var ok: Bool
    var text: String?
    var path: String?
    var hints: [HintItem]?
    var error: String?
}
