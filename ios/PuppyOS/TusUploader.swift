import Foundation

final class TusUploader {
    enum TusError: Error { case invalidEndpoint, createFailed, missingLocation, server(String), badResponse }

    let endpoint: URL
    let session: URLSession
    let headers: [String: String]
    let chunkSize: Int

    init(endpoint: URL, headers: [String: String] = [:], chunkSize: Int = 512*1024, session: URLSession = .shared) {
        self.endpoint = endpoint
        self.headers = headers
        self.chunkSize = max(64*1024, chunkSize)
        self.session = session
    }

    @discardableResult
    func upload(data: Data, filename: String, mimeType: String, resumeAt existingUploadURL: URL? = nil, onURL: ((URL) -> Void)? = nil, progress: @escaping (Int64, Int64) -> Void) async throws -> URL {
        let uploadURL: URL
        if let existing = existingUploadURL {
            uploadURL = existing
        } else {
            uploadURL = try await createUpload(length: data.count, filename: filename, mimeType: mimeType)
        }
        onURL?(uploadURL)

        // Query current offset (resume)
        var offset = try await headOffset(uploadURL: uploadURL)
        let total = Int64(data.count)
        while offset < total {
            let start = Int(offset)
            let end = min(start + chunkSize, data.count)
            let chunk = data[start..<end]
            offset = try await patch(uploadURL: uploadURL, offset: offset, chunk: Data(chunk))
            progress(offset, total)
        }
        return uploadURL
    }

    private func createUpload(length: Int, filename: String, mimeType: String) async throws -> URL {
        var req = URLRequest(url: endpoint)
        req.httpMethod = "POST"
        req.setValue("1.0.0", forHTTPHeaderField: "Tus-Resumable")
        req.setValue(String(length), forHTTPHeaderField: "Upload-Length")
        let meta = tusMetadata(["filename": filename, "content-type": mimeType])
        req.setValue(meta, forHTTPHeaderField: "Upload-Metadata")
        headers.forEach { req.setValue($0.value, forHTTPHeaderField: $0.key) }

        let (data, resp) = try await session.data(for: req)
        guard let http = resp as? HTTPURLResponse else { throw TusError.badResponse }
        guard (200..<300).contains(http.statusCode) else { throw TusError.server("create: \(http.statusCode) \(String(data: data, encoding: .utf8) ?? "")") }
        guard let locStr = http.allHeaderFields["Location"] as? String, let url = URL(string: locStr, relativeTo: endpoint) else { throw TusError.missingLocation }
        return url.absoluteURL
    }

    private func headOffset(uploadURL: URL) async throws -> Int64 {
        var req = URLRequest(url: uploadURL)
        req.httpMethod = "HEAD"
        req.setValue("1.0.0", forHTTPHeaderField: "Tus-Resumable")
        headers.forEach { req.setValue($0.value, forHTTPHeaderField: $0.key) }
        let (_, resp) = try await session.data(for: req)
        guard let http = resp as? HTTPURLResponse else { throw TusError.badResponse }
        if http.statusCode == 404 { return 0 }
        guard (200..<400).contains(http.statusCode) else { throw TusError.server("head: \(http.statusCode)") }
        let offsetStr = (http.allHeaderFields["Upload-Offset"] as? String) ?? "0"
        return Int64(offsetStr) ?? 0
    }

    private func patch(uploadURL: URL, offset: Int64, chunk: Data) async throws -> Int64 {
        var req = URLRequest(url: uploadURL)
        req.httpMethod = "PATCH"
        req.setValue("1.0.0", forHTTPHeaderField: "Tus-Resumable")
        req.setValue("application/offset+octet-stream", forHTTPHeaderField: "Content-Type")
        req.setValue(String(offset), forHTTPHeaderField: "Upload-Offset")
        req.httpBody = chunk
        headers.forEach { req.setValue($0.value, forHTTPHeaderField: $0.key) }
        let (data, resp) = try await session.data(for: req)
        guard let http = resp as? HTTPURLResponse else { throw TusError.badResponse }
        guard (200..<400).contains(http.statusCode) else { throw TusError.server("patch: \(http.statusCode) \(String(data: data, encoding: .utf8) ?? "")") }
        let offsetStr = (http.allHeaderFields["Upload-Offset"] as? String) ?? "0"
        return Int64(offsetStr) ?? 0
    }

    private func tusMetadata(_ dict: [String: String]) -> String {
        dict.map { key, value in
            let b64 = Data(value.utf8).base64EncodedString()
            return "\(key) \(b64)"
        }.joined(separator: ",")
    }
}
