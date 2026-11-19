import SwiftUI

struct SystemView: View {
    @Environment(Store.self) private var store

    var body: some View {
        Group {
            if let snap = store.snap {
                Form {
                    Section {
                        HStack(spacing: 12) {
                            Image(systemName: snap.system.ready ? "checkmark.seal.fill" : "exclamationmark.triangle.fill")
                                .font(.largeTitle)
                                .foregroundStyle(snap.system.ready ? Color.green : Color.orange)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(snap.system.ready ? "Máy đã sẵn sàng chạy game" : "Còn thiếu vài thứ")
                                    .font(.headline)
                                Text(snap.system.ready ? "Các mục bên dưới đều ổn." : "Xem các mục có dấu cảnh báo bên dưới.")
                                    .font(.callout)
                                    .foregroundStyle(.secondary)
                            }
                        }
                        .padding(.vertical, 4)
                    }
                    Section("Máy của bạn") {
                        ForEach(snap.system.checks) { c in
                            HStack(alignment: .top, spacing: 10) {
                                LevelIcon(level: c.level)
                                VStack(alignment: .leading, spacing: 1) {
                                    Text(c.label)
                                    Text(c.detail).font(.caption).foregroundStyle(.secondary)
                                }
                            }
                        }
                    }
                    NetSection(info: snap.net)
                }
                .formStyle(.grouped)
            } else {
                ProgressView()
            }
        }
        .navigationTitle("Hệ thống")
    }
}

struct NetSection: View {
    @Environment(Store.self) private var store
    let info: NetInfo

    var body: some View {
        Section {
            HStack {
                Text("Kiểm tra xem máy có vào được các máy chủ Steam và GitHub không. Hữu ích khi Steam báo \"cần trực tuyến\" dù Internet vẫn chạy.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
                Spacer()
                if info.busy {
                    ProgressView().controlSize(.small)
                } else {
                    Button(info.results == nil ? "Kiểm tra mạng" : "Kiểm tra lại") {
                        Task { await store.send(["cmd": "check_net"]) }
                    }
                }
            }
            if let results = info.results {
                ForEach(results) { r in
                    HStack(alignment: .top, spacing: 10) {
                        LevelIcon(level: r.level)
                        VStack(alignment: .leading, spacing: 1) {
                            Text(r.label)
                            Text(r.level == .ok ? r.why : r.detail)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            }
            if let advice = info.advice, !advice.isEmpty {
                ForEach(advice) { a in
                    HStack(alignment: .top, spacing: 10) {
                        LevelIcon(level: a.level)
                        Text(a.text).font(.callout)
                    }
                }
            }
        } header: {
            Text("Mạng")
        }
    }
}
