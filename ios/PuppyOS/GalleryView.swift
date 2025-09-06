import SwiftUI
import Photos
import UIKit

final class PhotosStore: ObservableObject {
    enum State {
        case idle
        case denied
        case loading
        case ready([PHAsset])
    }

    @Published var state: State = .idle
    private let imageManager = PHCachingImageManager()

    func requestAccessAndLoad() {
        let status = PHPhotoLibrary.authorizationStatus(for: .readWrite)
        switch status {
        case .authorized, .limited:
            loadAssets()
        case .notDetermined:
            PHPhotoLibrary.requestAuthorization(for: .readWrite) { [weak self] newStatus in
                DispatchQueue.main.async {
                    if newStatus == .authorized || newStatus == .limited {
                        self?.loadAssets()
                    } else {
                        self?.state = .denied
                    }
                }
            }
        default:
            state = .denied
        }
    }

    private func loadAssets() {
        state = .loading
        let options = PHFetchOptions()
        options.sortDescriptors = [NSSortDescriptor(key: "creationDate", ascending: false)]
        options.predicate = NSPredicate(format: "mediaType == %d", PHAssetMediaType.image.rawValue)
        let result = PHAsset.fetchAssets(with: options)
        var assets: [PHAsset] = []
        assets.reserveCapacity(result.count)
        result.enumerateObjects { asset, _, _ in assets.append(asset) }
        state = .ready(assets)
        // Prime caching small thumbnails for smoother scrolling
        imageManager.startCachingImages(for: assets, targetSize: CGSize(width: 200, height: 200), contentMode: .aspectFill, options: nil)
    }

    func requestThumbnail(for asset: PHAsset, targetSize: CGSize, completion: @escaping (UIImage?) -> Void) {
        let options = PHImageRequestOptions()
        options.deliveryMode = .opportunistic
        options.resizeMode = .fast
        options.isSynchronous = false
        imageManager.requestImage(for: asset, targetSize: targetSize, contentMode: .aspectFill, options: options) { image, _ in
            completion(image)
        }
    }

    func requestFullImage(for asset: PHAsset, completion: @escaping (UIImage?) -> Void) {
        let options = PHImageRequestOptions()
        options.deliveryMode = .highQualityFormat
        options.isNetworkAccessAllowed = true
        PHImageManager.default().requestImageDataAndOrientation(for: asset, options: options) { data, _, _, _ in
            if let data = data { completion(UIImage(data: data)) } else { completion(nil) }
        }
    }
}

struct GalleryView: View {
    @StateObject private var store = PhotosStore()
    @State private var selectedImage: UIImage? = nil
    @Environment(\.toggleMenu) private var toggleMenu
    @State private var showSync = false
    private let columns = Array(repeating: GridItem(.flexible(), spacing: 2), count: 3)

    var body: some View {
        NavigationStack {
            content
                .navigationTitle("Photos")
                .toolbar {
                    ToolbarItem(placement: .navigationBarLeading) {
                        Button { toggleMenu() } label: { Image(systemName: "line.3.horizontal") }
                            .accessibilityLabel("Open menu")
                    }
                    ToolbarItem(placement: .navigationBarTrailing) {
                        Button(action: refresh) { Image(systemName: "arrow.clockwise") }
                    }
                    ToolbarItem(placement: .primaryAction) {
                        Button { showSync = true } label: { Image(systemName: "arrow.up.circle") }
                            .accessibilityLabel("Sync photos")
                    }
                }
        }
        .onAppear { store.requestAccessAndLoad() }
        .sheet(isPresented: Binding(get: { selectedImage != nil }, set: { if !$0 { selectedImage = nil } })) {
            if let img = selectedImage {
                ImageViewer(image: img)
            }
        }
        .sheet(isPresented: $showSync) {
            PhotoSyncView()
        }
    }

    @ViewBuilder
    private var content: some View {
        switch store.state {
        case .idle, .loading:
            ProgressView("Loading Photos…").progressViewStyle(.circular)
        case .denied:
            VStack(spacing: 12) {
                Image(systemName: "photo")
                    .font(.largeTitle)
                    .foregroundColor(.secondary)
                Text("Photo Access Needed").font(.headline)
                Text("Enable photo access in Settings to view your gallery.")
                    .font(.subheadline).foregroundColor(.secondary).multilineTextAlignment(.center)
                Button("Open Settings", action: openSettings)
                    .buttonStyle(.borderedProminent)
            }.padding()
        case .ready(let assets):
            ScrollView {
                LazyVGrid(columns: columns, spacing: 2) {
                    ForEach(assets, id: \.localIdentifier) { asset in
                        ThumbnailCell(asset: asset) { img in
                            if let asset = img { selectedImage = asset }
                        }
                        .environmentObject(store)
                        .aspectRatio(1, contentMode: .fill)
                    }
                }.padding(2)
            }
        }
    }

    private func refresh() { store.requestAccessAndLoad() }

    private func openSettings() {
        if let url = URL(string: UIApplication.openSettingsURLString) {
            UIApplication.shared.open(url)
        }
    }
}

private struct ThumbnailCell: View {
    let asset: PHAsset
    var onSelect: (UIImage?) -> Void
    @State private var image: UIImage? = nil
    @EnvironmentObject private var store: PhotosStore

    var body: some View {
        ZStack {
            if let image = image {
                Image(uiImage: image).resizable().scaledToFill().clipped()
            } else {
                Color.secondary.opacity(0.15)
                ProgressView().progressViewStyle(.circular)
            }
        }
        .contentShape(Rectangle())
        .onTapGesture { selectFull() }
        .onAppear { loadThumb() }
    }

    private func loadThumb() {
        let scale = UIScreen.main.scale
        let size = CGSize(width: 120*scale, height: 120*scale)
        store.requestThumbnail(for: asset, targetSize: size) { img in
            DispatchQueue.main.async { self.image = img }
        }
    }

    private func selectFull() {
        store.requestFullImage(for: asset) { img in
            DispatchQueue.main.async { onSelect(img) }
        }
    }
}

private struct ImageViewer: View {
    let image: UIImage
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        NavigationStack {
            GeometryReader { proxy in
                Image(uiImage: image)
                    .resizable()
                    .scaledToFit()
                    .frame(width: proxy.size.width, height: proxy.size.height)
                    .background(Color.black.opacity(0.95))
                    .ignoresSafeArea()
            }
            .navigationTitle("Photo")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Close") { dismiss() } } }
        }
    }
}
