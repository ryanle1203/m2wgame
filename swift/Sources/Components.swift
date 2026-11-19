import AppKit
import SwiftUI
import UniformTypeIdentifiers

// Small reusable pieces that follow Apple's look.

extension Level {
    var symbol: String {
        switch self {
        case .ok: "checkmark.circle.fill"
        case .warn: "exclamationmark.triangle.fill"
        case .bad: "xmark.octagon.fill"
        }
    }

    var color: Color {
        switch self {
        case .ok: .green
        case .warn: .orange
        case .bad: .red
        }
    }
}

struct LevelIcon: View {
    let level: Level
    var body: some View {
        Image(systemName: level.symbol)
            .foregroundStyle(level.color)
            .imageScale(.large)
            .accessibilityLabel(level == .ok ? L("Tốt") : level == .warn ? L("Lưu ý") : L("Cần sửa"))
    }
}

/// Rounded "app icon" tile with the first letter of the name.
struct AppTile: View {
    let name: String
    var size: CGFloat = 44

    private static let palette: [Color] = [.blue, .indigo, .purple, .pink, .orange, .teal, .green, .mint]

    private var tint: Color {
        let sum = name.unicodeScalars.reduce(0) { $0 &+ Int($1.value) }
        return Self.palette[sum % Self.palette.count]
    }

    var body: some View {
        RoundedRectangle(cornerRadius: size * 0.225, style: .continuous)
            .fill(LinearGradient(colors: [tint.opacity(0.85), tint], startPoint: .top, endPoint: .bottom))
            .frame(width: size, height: size)
            .overlay {
                Text(String(name.trimmingCharacters(in: .whitespaces).prefix(1)).uppercased())
                    .font(.system(size: size * 0.46, weight: .semibold, design: .rounded))
                    .foregroundStyle(.white)
            }
            .accessibilityHidden(true)
    }
}

struct Badge: View {
    let text: String
    var color: Color = .secondary

    var body: some View {
        Text(LocalizedStringKey(text))
            .font(.caption2.weight(.semibold))
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .foregroundStyle(color)
            .background(color.opacity(0.14), in: Capsule())
    }
}

/// Sheet footer: Cancel on the left of the default action, like macOS dialogs.
struct SheetButtons: View {
    var cancelTitle = L("Hủy")
    let actionTitle: String
    var actionEnabled = true
    var destructive = false
    let onCancel: () -> Void
    let onAction: () -> Void

    var body: some View {
        HStack {
            Spacer()
            Button(LocalizedStringKey(cancelTitle), action: onCancel)
                .keyboardShortcut(.cancelAction)
            Button(LocalizedStringKey(actionTitle), role: destructive ? .destructive : nil, action: onAction)
                .keyboardShortcut(.defaultAction)
                .buttonStyle(.borderedProminent)
                .disabled(!actionEnabled)
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 14)
    }
}

/// A small text prompt (new environment, rename).
struct NameSheet: View {
    let title: String
    let message: String
    let actionTitle: String
    @State var text: String
    @FocusState private var focused: Bool
    let onDone: (String) -> Void
    @Environment(\.dismiss) private var dismiss

    init(title: String, message: String, actionTitle: String, initial: String = "", onDone: @escaping (String) -> Void) {
        self.title = title
        self.message = message
        self.actionTitle = actionTitle
        self._text = State(initialValue: initial)
        self.onDone = onDone
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(LocalizedStringKey(title)).font(.headline)
            Text(LocalizedStringKey(message)).font(.subheadline).foregroundStyle(.secondary)
            TextField("Tên", text: $text)
                .textFieldStyle(.roundedBorder)
                .focused($focused)
                .onSubmit(commit)
                .onAppear { focused = true }
        }
        .padding(20)
        SheetButtons(actionTitle: actionTitle, actionEnabled: !text.trimmingCharacters(in: .whitespaces).isEmpty,
                     onCancel: { dismiss() }, onAction: commit)
            .frame(width: 400)
    }

    private func commit() {
        let t = text.trimmingCharacters(in: .whitespaces)
        guard !t.isEmpty else { return }
        onDone(t)
        dismiss()
    }
}

/// Ask macOS for a file. Returns its path, or nil if the user cancelled.
@MainActor
func chooseFile(message: String, extensions: [String] = [], insideBundles: Bool = false) -> String? {
    let panel = NSOpenPanel()
    panel.message = message
    panel.canChooseFiles = true
    panel.canChooseDirectories = false
    panel.allowsMultipleSelection = false
    panel.treatsFilePackagesAsDirectories = insideBundles
    if !extensions.isEmpty {
        panel.allowedContentTypes = extensions.compactMap { UTType(filenameExtension: $0) }
    }
    return panel.runModal() == .OK ? panel.url?.path : nil
}

func reveal(_ path: String) {
    NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
}

func formatElapsed(_ secs: Int) -> String {
    String(format: "%d:%02d", secs / 60, secs % 60)
}

func formatBytes(_ bytes: Int64) -> String {
    ByteCountFormatter.string(fromByteCount: bytes, countStyle: .file)
}
