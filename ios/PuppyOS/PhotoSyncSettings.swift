import Foundation

struct PhotoSyncSettings: Codable {
    var endpoint: String
    var authHeaderKey: String?
    var authHeaderValue: String?
    var chunkSize: Int

    static let `default` = PhotoSyncSettings(endpoint: "", authHeaderKey: nil, authHeaderValue: nil, chunkSize: 512 * 1024)

    private static let key = "PhotoSyncSettings"

    static func load() -> PhotoSyncSettings {
        let ud = UserDefaults.standard
        if let data = ud.data(forKey: key), let s = try? JSONDecoder().decode(PhotoSyncSettings.self, from: data) { return s }
        return .default
    }

    func save() {
        if let data = try? JSONEncoder().encode(self) {
            UserDefaults.standard.set(data, forKey: PhotoSyncSettings.key)
        }
    }
}

