import Foundation

/// Interface language. Vietnamese is the source language: the texts in the code are Vietnamese
/// and double as the lookup keys; the English texts are in `Resources/en.lproj/Localizable.strings`.
enum LanguageChoice: String, CaseIterable, Identifiable {
    case system, en, vi

    var id: String { rawValue }

    /// Shown in the language menu. Not translated: each language is named in itself.
    var title: String {
        switch self {
        case .system: "Auto / Tự động"
        case .en: "English"
        case .vi: "Tiếng Việt"
        }
    }

    /// "en" or "vi".
    var code: String {
        switch self {
        case .en: "en"
        case .vi: "vi"
        case .system: Locale.preferredLanguages.first?.lowercased().hasPrefix("vi") == true ? "vi" : "en"
        }
    }

    private static let key = "M2WLanguage"

    /// The saved choice; `M2WGAME_LANG=en|vi` (used for screenshots and tests) wins over it.
    static func loaded() -> LanguageChoice {
        if let env = ProcessInfo.processInfo.environment["M2WGAME_LANG"], let c = LanguageChoice(rawValue: env) {
            return c
        }
        if let raw = UserDefaults.standard.string(forKey: key), let c = LanguageChoice(rawValue: raw) {
            return c
        }
        return .system
    }

    func save() {
        UserDefaults.standard.set(rawValue, forKey: Self.key)
    }
}

/// The language the interface currently speaks ("en" or "vi"). Set by the `Store`.
nonisolated(unsafe) var activeLanguage = "vi"

private let englishTexts: [String: String] = {
    guard let url = Bundle.main.url(forResource: "Localizable", withExtension: "strings", subdirectory: nil, localization: "en"),
          let dict = NSDictionary(contentsOf: url) as? [String: String] else { return [:] }
    return dict
}()

/// Translate a Vietnamese text that is not shown through SwiftUI's own localization
/// (plain `String`s: computed labels, alerts, messages).
func L(_ vi: String) -> String {
    activeLanguage == "en" ? (englishTexts[vi] ?? vi) : vi
}

/// `L` for texts with `%@` placeholders, filled in order.
func L(_ vi: String, _ args: CVarArg...) -> String {
    String(format: L(vi), arguments: args)
}
