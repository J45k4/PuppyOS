import SwiftUI
import UniformTypeIdentifiers
import UIKit
import MobileCoreServices

private let selectedFolderBookmarkKey = "SelectedFolderBookmark"

struct FileBrowserView: View {
    @Environment(\.toggleMenu) private var toggleMenu
    @State private var documents: [URL] = []
    @State private var isImporterPresented = false
    @State private var isFolderPickerPresented = false
    @State private var importError: String? = nil
    @State private var showAlert = false
    @State private var toShare: URL?
    @State private var selectedFolderURL: URL? = nil
    @State private var isDocPickerPresented = false
    @State private var pdfURL: URL? = nil

    var body: some View {
        NavigationStack {
            Group {
                if documents.isEmpty {
                    if #available(iOS 17.0, *) {
                        ContentUnavailableView("No Files", systemImage: "folder", description: Text("Import files from the Files app to view and manage them here."))
                    } else {
                        VStack(spacing: 12) {
                            Image(systemName: "folder")
                                .font(.largeTitle)
                                .foregroundColor(.secondary)
                            Text("No Files")
                                .font(.headline)
                            Text("Import files from the Files app to view and manage them here.")
                                .font(.subheadline)
                                .multilineTextAlignment(.center)
                                .foregroundColor(.secondary)
                                .padding(.horizontal)
                        }
                    }
                } else {
                    List {
                        if let folder = selectedFolderURL {
                            Section(header: Text(folderDisplayName(folder))) {}
                        }
                        ForEach(documents, id: \.self) { url in
                            if isDirectory(url) {
                                NavigationLink(destination: FolderContentsView(rootFolder: url, onOpen: { handleOpen($0) }, onShare: { item in toShare = item }, onDelete: { delete($0) })) {
                                    RowView(url: url)
                                }
                            } else {
                                RowView(url: url)
                                    .swipeActions(edge: .trailing, allowsFullSwipe: true) {
                                        Button(role: .destructive) { delete(url) } label: {
                                            Label("Delete", systemImage: "trash")
                                        }
                                    }
                                    .contextMenu {
                                        if #available(iOS 16.0, *) {
                                            ShareLink("Share", item: url)
                                        }
                                        Button(role: .destructive) { delete(url) } label: { Label("Delete", systemImage: "trash") }
                                    }
                                    .contentShape(Rectangle())
                                    .onTapGesture { handleOpen(url) }
                            }
                        }
                    }
                    .listStyle(.plain)
                }
            }
            .navigationTitle("Files")
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button { toggleMenu() } label: { Image(systemName: "line.3.horizontal") }
                        .accessibilityLabel("Open menu")
                }
                ToolbarItem(placement: .primaryAction) {
                    Button {
                        isImporterPresented = true
                    } label: {
                        Label("Import", systemImage: "square.and.arrow.down")
                    }
                    .accessibilityLabel("Import from Files")
                }
                ToolbarItemGroup(placement: .bottomBar) {
                    Button {
                        isFolderPickerPresented = true
                    } label: {
                        Label(selectedFolderURL == nil ? "Browse Device" : "Change Folder", systemImage: "folder.badge.plus")
                    }
                    if selectedFolderURL != nil {
                        Button(role: .destructive) { clearSelectedFolder() } label: {
                            Label("Stop Browsing", systemImage: "xmark.circle")
                        }
                    }
                    Button { isDocPickerPresented = true } label: {
                        Label("Browse Files", systemImage: "doc")
                    }
                    Spacer()
                }
            }
        }
        .fileImporter(
            isPresented: $isImporterPresented,
            allowedContentTypes: [.item], // allow any file type
            allowsMultipleSelection: true
        ) { result in
            switch result {
            case .success(let urls):
                importError = nil
                importFiles(urls)
            case .failure(let error):
                importError = error.localizedDescription
                showAlert = true
            }
        }
        .fileImporter(
            isPresented: $isFolderPickerPresented,
            allowedContentTypes: [.folder],
            allowsMultipleSelection: false
        ) { result in
            switch result {
            case .success(let urls):
                if let folder = urls.first {
                    setSelectedFolder(folder)
                }
            case .failure(let error):
                importError = error.localizedDescription
                showAlert = true
            }
        }
        .onAppear(perform: reload)
        .alert("Import Error", isPresented: $showAlert, presenting: importError) { _ in
            Button("OK", role: .cancel) {}
        } message: { err in
            Text(err)
        }
        // Fallback share sheet for iOS 15 or to preview
        .sheet(isPresented: Binding(get: { toShare != nil }, set: { if !$0 { toShare = nil } })) {
            if let item = toShare { ActivityView(activityItems: [item]) }
        }
        .sheet(isPresented: $isDocPickerPresented) {
            DocumentPickerView { urls in
                // Immediately present the first item for preview/share
                if let first = urls.first { toShare = first }
            }
        }
        .sheet(item: Binding(get: { pdfURL.map(IdentifiedURL.init(url:)) }, set: { pdfURL = $0?.url })) { wrapper in
            PDFViewerScreen(url: wrapper.url)
        }
    }

    private func reload() {
        // Try load selected folder from bookmark if not already loaded
        if selectedFolderURL == nil, let data = UserDefaults.standard.data(forKey: selectedFolderBookmarkKey) {
            do {
                var stale = false
                let url = try URL(resolvingBookmarkData: data, options: [], relativeTo: nil, bookmarkDataIsStale: &stale)
                selectedFolderURL = url
            } catch {
                // ignore; fallback to Documents
            }
        }
        let fm = FileManager.default
        let dir = selectedFolderURL ?? documentsDirectory()
        var didStart = false
        if selectedFolderURL != nil { didStart = dir.startAccessingSecurityScopedResource() }
        defer { if didStart { dir.stopAccessingSecurityScopedResource() } }
        let contents = (try? fm.contentsOfDirectory(at: dir, includingPropertiesForKeys: [.contentModificationDateKey, .fileAllocatedSizeKey, .isDirectoryKey], options: [.skipsHiddenFiles])) ?? []
        documents = contents.sorted(by: fileSort)
    }

    private func importFiles(_ urls: [URL]) {
        let fm = FileManager.default
        let destDir = documentsDirectory()
        for src in urls {
            let dest = uniqueDestination(for: src.lastPathComponent, in: destDir)
            do {
                // Start accessing security-scoped resource if needed
                let needsStop = src.startAccessingSecurityScopedResource()
                defer { if needsStop { src.stopAccessingSecurityScopedResource() } }
                if fm.fileExists(atPath: dest.path) { try? fm.removeItem(at: dest) }
                try fm.copyItem(at: src, to: dest)
            } catch {
                importError = "Failed to import \(src.lastPathComponent): \(error.localizedDescription)"
                showAlert = true
            }
        }
        reload()
    }

    private func delete(_ url: URL) {
        do {
            var didStart = false
            if selectedFolderURL != nil { didStart = url.startAccessingSecurityScopedResource() }
            defer { if didStart { url.stopAccessingSecurityScopedResource() } }
            try FileManager.default.removeItem(at: url)
            reload()
        } catch {
            importError = "Failed to delete \(url.lastPathComponent): \(error.localizedDescription)"
            showAlert = true
        }
    }

    private func documentsDirectory() -> URL {
        FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
    }

    private func handleOpen(_ url: URL) {
        if url.pathExtension.lowercased() == "pdf" {
            pdfURL = url
        } else {
            toShare = url
        }
    }

    private func setSelectedFolder(_ folder: URL) {
        do {
            let data = try folder.bookmarkData(options: [], includingResourceValuesForKeys: nil, relativeTo: nil)
            UserDefaults.standard.set(data, forKey: selectedFolderBookmarkKey)
            selectedFolderURL = folder
            reload()
        } catch {
            importError = "Failed to bookmark folder: \(error.localizedDescription)"
            showAlert = true
        }
    }

    private func clearSelectedFolder() {
        UserDefaults.standard.removeObject(forKey: selectedFolderBookmarkKey)
        selectedFolderURL = nil
        reload()
    }

    private func uniqueDestination(for filename: String, in dir: URL) -> URL {
        var candidate = dir.appendingPathComponent(filename)
        let base = (filename as NSString).deletingPathExtension
        let ext = (filename as NSString).pathExtension
        var idx = 1
        while FileManager.default.fileExists(atPath: candidate.path) {
            let newName = base + " (" + String(idx) + ")" + (ext.isEmpty ? "" : "." + ext)
            candidate = dir.appendingPathComponent(newName)
            idx += 1
        }
        return candidate
    }

    private func folderDisplayName(_ folder: URL) -> String {
        if folder.path.contains("/Documents") { return "App Documents" }
        return folder.lastPathComponent
    }

    
}

// Simple UIActivityViewController wrapper for share/preview
private struct ActivityView: UIViewControllerRepresentable {
    let activityItems: [Any]
    let applicationActivities: [UIActivity]? = nil

    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: activityItems, applicationActivities: applicationActivities)
    }

    func updateUIViewController(_ uiViewController: UIActivityViewController, context: Context) {}
}

// Reusable row for files/folders
private struct RowView: View {
    let url: URL
    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: iconName(for: url))
                .foregroundColor(isDirectory(url) ? .orange : .blue)
            VStack(alignment: .leading, spacing: 2) {
                Text(url.lastPathComponent)
                    .lineLimit(1)
                Text(metaString(for: url))
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            Spacer()
            if !isDirectory(url) {
                if #available(iOS 16.0, *) {
                    ShareLink(item: url) {
                        Image(systemName: "square.and.arrow.up")
                    }
                    .buttonStyle(.borderless)
                }
            } else {
                Image(systemName: "chevron.right")
                    .foregroundColor(Color(UIColor.tertiaryLabel))
            }
        }
    }
}

// View to display a subfolder's contents
private struct FolderContentsView: View {
    let rootFolder: URL
    var onOpen: (URL) -> Void
    var onShare: (URL) -> Void
    var onDelete: (URL) -> Void
    @State private var items: [URL] = []

    var body: some View {
        List {
            ForEach(items, id: \.self) { url in
                if isDirectory(url) {
                    NavigationLink(destination: FolderContentsView(rootFolder: url, onOpen: onOpen, onShare: onShare, onDelete: onDelete)) {
                        RowView(url: url)
                    }
                } else {
                    RowView(url: url)
                        .swipeActions(edge: .trailing, allowsFullSwipe: true) {
                            Button(role: .destructive) { onDelete(url) } label: { Label("Delete", systemImage: "trash") }
                        }
                        .contextMenu {
                            if #available(iOS 16.0, *) { ShareLink("Share", item: url) }
                            Button(role: .destructive) { onDelete(url) } label: { Label("Delete", systemImage: "trash") }
                        }
                        .contentShape(Rectangle())
                        .onTapGesture { onOpen(url) }
                }
            }
        }
        .navigationTitle(rootFolder.lastPathComponent)
        .onAppear(perform: reload)
    }

    private func reload() {
        var didStart = false
        didStart = rootFolder.startAccessingSecurityScopedResource()
        defer { if didStart { rootFolder.stopAccessingSecurityScopedResource() } }
        let fm = FileManager.default
        let contents = (try? fm.contentsOfDirectory(at: rootFolder, includingPropertiesForKeys: [.contentModificationDateKey, .fileAllocatedSizeKey, .isDirectoryKey], options: [.skipsHiddenFiles])) ?? []
        items = contents.sorted(by: fileSort)
    }
}

// MARK: - Helpers

fileprivate func iconName(for url: URL) -> String {
    if isDirectory(url) { return "folder" }
    let ext = url.pathExtension.lowercased()
    switch ext {
    case "txt", "md": return "doc.text"
    case "json": return "curlybraces"
    case "jpg", "jpeg", "png", "gif", "heic", "svg": return "photo"
    case "pdf": return "doc.richtext"
    case "zip", "gz", "tar", "7z", "rar": return "archivebox"
    case "mp3", "wav", "m4a": return "music.note"
    case "mp4", "mov", "m4v": return "film"
    default: return "doc"
    }
}

fileprivate func metaString(for url: URL) -> String {
    let values = try? url.resourceValues(forKeys: [.fileSizeKey, .contentModificationDateKey, .isDirectoryKey])
    let isDir = values?.isDirectory ?? false
    var parts: [String] = []
    if !isDir, let size = values?.fileSize {
        let formatter = ByteCountFormatter()
        formatter.countStyle = .file
        parts.append(formatter.string(fromByteCount: Int64(size)))
    }
    if let date = values?.contentModificationDate {
        let df = DateFormatter()
        df.dateStyle = .short
        df.timeStyle = .short
        parts.append(df.string(from: date))
    }
    return parts.joined(separator: " · ")
}

fileprivate func isDirectory(_ url: URL) -> Bool {
    (try? url.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) ?? false
}

fileprivate func fileSort(_ lhs: URL, _ rhs: URL) -> Bool {
    let ld = isDirectory(lhs)
    let rd = isDirectory(rhs)
    if ld != rd { return ld && !rd }
    let l = (try? lhs.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate) ?? Date.distantPast
    let r = (try? rhs.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate) ?? Date.distantPast
    return l > r
}

// Helper for sheet(item:) with URL
private struct IdentifiedURL: Identifiable {
    let id: String
    let url: URL
    init(url: URL) {
        self.url = url
        self.id = url.absoluteString
    }
}

// System Files-style document picker (open in place)
private struct DocumentPickerView: UIViewControllerRepresentable {
    var onPick: ([URL]) -> Void

    func makeUIViewController(context: Context) -> UIDocumentPickerViewController {
        let picker = UIDocumentPickerViewController(forOpeningContentTypes: [UTType.item], asCopy: false)
        picker.allowsMultipleSelection = true
        picker.delegate = context.coordinator
        return picker
    }

    func updateUIViewController(_ uiViewController: UIDocumentPickerViewController, context: Context) {}

    func makeCoordinator() -> Coordinator { Coordinator(onPick: onPick) }

    final class Coordinator: NSObject, UIDocumentPickerDelegate {
        let onPick: ([URL]) -> Void
        init(onPick: @escaping ([URL]) -> Void) { self.onPick = onPick }
        func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
            onPick(urls)
        }
        func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {}
    }
}
