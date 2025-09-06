import Foundation
import Photos

final class PhotoSyncManager: ObservableObject {
    enum SyncState { case idle, syncing(Double, String), done, error(String) }
    @Published var state: SyncState = .idle

    private let store = PhotosStore()
    private let settings: PhotoSyncSettings

    // Map of assetId -> uploadURL for resume
    private let uploadsKey = "PhotoSyncUploads"
    private var uploadMap: [String: String] { // id -> URL string
        get { (UserDefaults.standard.dictionary(forKey: uploadsKey) as? [String: String]) ?? [:] }
        set { UserDefaults.standard.set(newValue, forKey: uploadsKey) }
    }

    init(settings: PhotoSyncSettings) { self.settings = settings }

    func syncRecent(limit: Int = 20) {
        guard let endpoint = URL(string: settings.endpoint), !settings.endpoint.isEmpty else {
            state = .error("Configure endpoint URL first")
            return
        }
        Task { @MainActor in
            self.state = .syncing(0, "Preparing…")
        }
        Task {
            let headers: [String: String] = {
                if let k = settings.authHeaderKey, let v = settings.authHeaderValue, !k.isEmpty, !v.isEmpty { return [k: v] }
                return [:]
            }()
            let uploader = TusUploader(endpoint: endpoint, headers: headers, chunkSize: settings.chunkSize)
            let assets = fetchRecentPhotos(limit: limit)
            let total = Double(assets.count)
            var index = 0.0
            for asset in assets {
                index += 1
                let title = (asset.value(forKey: "filename") as? String) ?? asset.localIdentifier
                await MainActor.run { self.state = .syncing(index/total, "Uploading \(title)") }
                do {
                    let (data, filename, mime) = try await loadData(for: asset)
                    let existingURL = uploadMap[asset.localIdentifier].flatMap { URL(string: $0) }
                    let usedURL = try await uploader.upload(
                        data: data,
                        filename: filename,
                        mimeType: mime,
                        resumeAt: existingURL,
                        onURL: { url in
                            var map = self.uploadMap
                            map[asset.localIdentifier] = url.absoluteString
                            self.uploadMap = map
                        },
                        progress: { sent, totalBytes in }
                    )
                    // Success; clear mapping
                    var doneMap = uploadMap
                    doneMap.removeValue(forKey: asset.localIdentifier)
                    uploadMap = doneMap
                } catch {
                    // On create, try to store returned Location if we can parse
                    // TusUploader handles creation; on failure, leave mapping as-is
                    await MainActor.run { self.state = .error("Failed: \(title): \(error.localizedDescription)") }
                    return
                }
            }
            await MainActor.run { self.state = .done }
        }
    }

    private func fetchRecentPhotos(limit: Int) -> [PHAsset] {
        let options = PHFetchOptions()
        options.sortDescriptors = [NSSortDescriptor(key: "creationDate", ascending: false)]
        options.predicate = NSPredicate(format: "mediaType == %d", PHAssetMediaType.image.rawValue)
        options.fetchLimit = limit
        let result = PHAsset.fetchAssets(with: options)
        var assets: [PHAsset] = []
        assets.reserveCapacity(result.count)
        result.enumerateObjects { asset, _, stop in assets.append(asset) }
        return assets
    }

    private func loadData(for asset: PHAsset) async throws -> (Data, String, String) {
        // Prefer original resource
        if let res = PHAssetResource.assetResources(for: asset).first {
            let filename = res.originalFilename
            let mime = utiToMime(res.uniformTypeIdentifier) ?? "image/jpeg"
            let data = try await readResource(res)
            return (data, filename, mime)
        }
        // Fallback to image data
        return try await withCheckedThrowingContinuation { cont in
            let opts = PHImageRequestOptions()
            opts.deliveryMode = .highQualityFormat
            opts.isNetworkAccessAllowed = true
            PHImageManager.default().requestImageDataAndOrientation(for: asset, options: opts) { maybeData, uti, _, _ in
                if let d = maybeData, let uti = uti {
                    let filename = asset.localIdentifier.replacingOccurrences(of: "/", with: "_") + ".jpg"
                    let mime = utiToMime(uti) ?? "image/jpeg"
                    cont.resume(returning: (d, filename, mime))
                } else {
                    cont.resume(throwing: NSError(domain: "PhotoSync", code: -1, userInfo: [NSLocalizedDescriptionKey: "Image data unavailable"]))
                }
            }
        }
    }

    private func readResource(_ res: PHAssetResource) async throws -> Data {
        try await withCheckedThrowingContinuation { cont in
            let buff = NSMutableData()
            let opt = PHAssetResourceRequestOptions()
            opt.isNetworkAccessAllowed = true
            PHAssetResourceManager.default().requestData(for: res, options: opt) { chunk in
                buff.append(chunk)
            } completionHandler: { error in
                if let error { cont.resume(throwing: error) }
                else { cont.resume(returning: buff as Data) }
            }
        }
    }
}

private func utiToMime(_ uti: String) -> String? {
    if uti.contains("jpeg") || uti.contains("jpg") { return "image/jpeg" }
    if uti.contains("png") { return "image/png" }
    if uti.contains("heic") { return "image/heic" }
    if uti.contains("gif") { return "image/gif" }
    return nil
}
