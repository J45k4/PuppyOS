import SwiftUI

struct PhotoSyncView: View {
    @State private var settings = PhotoSyncSettings.load()
    @StateObject private var manager = PhotoSyncManager(settings: PhotoSyncSettings.load())
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Form {
                Section("Server") {
                    TextField("TUS endpoint URL", text: $settings.endpoint)
                        .autocapitalization(.none)
                        .keyboardType(.URL)
                        .textContentType(.URL)
                    HStack {
                        TextField("Auth header key (e.g. Authorization)", text: Binding(get: { settings.authHeaderKey ?? "" }, set: { settings.authHeaderKey = $0.isEmpty ? nil : $0 }))
                        Text(":")
                        TextField("Value (e.g. Bearer …)", text: Binding(get: { settings.authHeaderValue ?? "" }, set: { settings.authHeaderValue = $0.isEmpty ? nil : $0 }))
                    }
                }
                Section("Advanced") {
                    Stepper("Chunk size: \(settings.chunkSize/1024) KB", value: $settings.chunkSize, in: 64*1024...4*1024*1024, step: 64*1024)
                }
                Section("Sync") {
                    Button {
                        settings.save()
                        manager.state = .idle
                        let s = PhotoSyncSettings.load()
                        let m = PhotoSyncManager(settings: s)
                        _manager.wrappedValue = m
                        m.syncRecent(limit: 20)
                    } label: {
                        Label("Sync last 20 photos", systemImage: "arrow.up.circle")
                    }
                }
                Section("Status") {
                    switch manager.state {
                    case .idle:
                        Text("Idle")
                    case .syncing(let p, let msg):
                        VStack(alignment: .leading) {
                            ProgressView(value: p)
                            Text(msg).font(.footnote).foregroundColor(.secondary)
                        }
                    case .done:
                        Text("Completed successfully").foregroundColor(.green)
                    case .error(let err):
                        Text(err).foregroundColor(.red)
                    }
                }
            }
            .navigationTitle("Photo Sync")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Close") { dismiss() } }
            }
        }
    }
}

