import UIKit
import WebKit
import UniformTypeIdentifiers
import QuickLook
import CryptoKit
import AVFoundation
import VLCKit

/// Security-scoped folder access; no access outside folders chosen in Files.
@MainActor
@objc(MediaManagerStorage)
public final class MediaManagerStorage: NSObject, WKScriptMessageHandler, UIDocumentPickerDelegate, QLPreviewControllerDataSource, QLPreviewControllerDelegate {
    private weak var controller: UIViewController?
    private weak var browser: WKWebView?
    private var pending: [String: Any]?
    private var previewURL: URL?
    private var previewRoot: URL?

    public override init() { super.init() }

    @objc public func attachStorage(_ address: UInt64) {
        guard let pointer = UnsafeRawPointer(bitPattern: UInt(address)) else { return }
        let host = Unmanaged<UIViewController>.fromOpaque(pointer).takeUnretainedValue()
        controller = host; browser = findWebView(host.view)
        browser?.configuration.userContentController.add(self, name: "MediaManagerStorage")
    }
    @objc public func detachStorage() {
        browser?.configuration.userContentController.removeScriptMessageHandler(forName: "MediaManagerStorage")
        previewRoot?.stopAccessingSecurityScopedResource(); previewRoot = nil
        browser = nil; controller = nil
    }
    private func findWebView(_ view: UIView) -> WKWebView? {
        if let web = view as? WKWebView { return web }
        for child in view.subviews { if let web = findWebView(child) { return web } }
        return nil
    }
    private func fail(_ message: String) -> NSError { NSError(domain: "MediaManagerStorage", code: 1, userInfo: [NSLocalizedDescriptionKey: message]) }
    private func reply(_ command: [String: Any], _ result: [String: Any]) {
        var value = result; value["id"] = command["id"]
        guard let bytes = try? JSONSerialization.data(withJSONObject: value), let text = String(data: bytes, encoding: .utf8) else { return }
        browser?.evaluateJavaScript("window.dispatchEvent(new CustomEvent('media-manager-storage',{detail:\(text)}))")
    }
    public func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard message.frameInfo.isMainFrame, let url = message.frameInfo.request.url,
              url.scheme == "dioxus", url.host == "index.html",
              let text = message.body as? String, let bytes = text.data(using: .utf8),
              let command = (try? JSONSerialization.jsonObject(with: bytes)) as? [String: Any] else { return }
        if command["op"] as? String == "appDirectories" {
            do {
                let state = try FileManager.default.url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
                reply(command, ["state": state.path, "cache": FileManager.default.temporaryDirectory.path])
            } catch { reply(command, ["error": error.localizedDescription]) }
            return
        }
        if command["op"] as? String == "scanPairing" {
            guard pending == nil, let controller, controller.presentedViewController == nil else { reply(command, ["error": "Close the current dialog before scanning"]); return }
            pending = command
            AVCaptureDevice.requestAccess(for: .video) { [weak self] granted in
                Task { @MainActor in
                    guard let self, self.pending != nil else { return }
                    guard granted else { self.pending = nil; self.reply(command, ["error": "Camera permission denied. Enable Camera access in Settings to scan."]); return }
                    let scanner = PairingCameraController { [weak self] result in
                        guard let self else { return }; self.pending = nil
                        switch result { case .success(let text): self.reply(command, ["text": text]); case .failure(let error): self.reply(command, ["error": error.localizedDescription]) }
                    }
                    scanner.modalPresentationStyle = .fullScreen
                    controller.present(scanner, animated: true)
                }
            }
            return
        }
        if command["op"] as? String == "pick" || command["op"] as? String == "pickImage" {
            guard pending == nil, let controller, controller.presentedViewController == nil else { reply(command, ["error": "Close the current dialog before choosing a folder"]); return }
            pending = command
            let picker = UIDocumentPickerViewController(forOpeningContentTypes: command["op"] as? String == "pickImage" ? [.image] : [.folder], asCopy: false)
            picker.delegate = self; picker.allowsMultipleSelection = false
            controller.present(picker, animated: true)
            return
        }
        // File coordination and cloud-provider I/O must not block the UI thread.
        let operation = command["op"] as? String ?? ""
        if operation == "playCompatibleCache" {
            do {
                let file=URL(fileURLWithPath:command["path"] as? String ?? "").resolvingSymlinksInPath()
                let cache=FileManager.default.temporaryDirectory.resolvingSymlinksInPath().path+"/"
                guard file.path.hasPrefix(cache),file.lastPathComponent=="preview.mp4",let controller,controller.presentedViewController == nil else {throw fail("Invalid compatibility preview or another dialog is open")}
                let preview=CompatibilityPlayerController(url:file) {}
                preview.modalPresentationStyle = .fullScreen
                controller.present(preview,animated:true);reply(command,[:])
            }catch{reply(command,["error":error.localizedDescription])}
            return
        }
        if operation == "play" || operation == "playCompatiblePreview" {
            do {
                let root = try folder(command)
                let file: URL
                do { file = try resolve(root, command["path"] as? String ?? "") }
                catch { root.stopAccessingSecurityScopedResource(); throw error }
                previewRoot?.stopAccessingSecurityScopedResource(); previewRoot = root; previewURL = file
                if operation == "playCompatiblePreview" {
                    guard let controller, controller.presentedViewController == nil else { throw fail("Close the current player before opening another") }
                    let preview = CompatibilityPlayerController(url: file) { [weak self] in
                        self?.previewRoot?.stopAccessingSecurityScopedResource(); self?.previewRoot = nil
                    }
                    preview.modalPresentationStyle = .fullScreen
                    controller.present(preview, animated: true); reply(command, [:]); return
                }
                let preview = QLPreviewController(); preview.dataSource = self; preview.delegate = self
                controller?.present(preview, animated: true); reply(command, [:])
            } catch { reply(command, ["error": error.localizedDescription]) }
            return
        }
        // Native filesystem operations are serialized, including mutations.
        let root: URL
        do { root = try folder(command) } catch { reply(command, ["error": error.localizedDescription]); return }
        Self.queue.async {
            let result: [String: Any]
            do { result = try Self.execute(command, root) }
            catch { result = ["error": error.localizedDescription] }
            root.stopAccessingSecurityScopedResource()
            DispatchQueue.main.async { self.reply(command, result) }
        }
    }
    private static let queue = DispatchQueue(label: "MediaManagerStorage")
    public func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
        if let command = pending { reply(command, ["error": "Folder selection cancelled"]) }; pending = nil
    }
    public func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        guard let command = pending else { return }; pending = nil
        do {
            guard let url = urls.first, url.startAccessingSecurityScopedResource() else { throw fail("Folder permission was not granted") }
            defer { url.stopAccessingSecurityScopedResource() }
            if command["op"] as? String == "pickImage" {
                let values=try url.resourceValues(forKeys:[.fileSizeKey])
                guard (values.fileSize ?? Int.max) <= 10 * 1024 * 1024 else { throw fail("QR image exceeds 10 MB") }
                let bytes=try Data(contentsOf:url)
                reply(command,["data":bytes.base64EncodedString()]); return
            }
            let bookmark = try url.bookmarkData(options: [.minimalBookmark], includingResourceValuesForKeys: nil, relativeTo: nil)
            let key = UUID().uuidString
            UserDefaults.standard.set(bookmark, forKey: "MediaManagerFolder-" + key)
            reply(command, ["root": key])
        } catch { reply(command, ["error": error.localizedDescription]) }
    }
    private func folder(_ command: [String: Any]) throws -> URL {
        guard let key = command["root"] as? String, UUID(uuidString: key) != nil,
              let bytes = UserDefaults.standard.data(forKey: "MediaManagerFolder-" + key) else { throw fail("Choose a media folder first") }
        var stale = false
        let url = try URL(resolvingBookmarkData: bytes, options: [], relativeTo: nil, bookmarkDataIsStale: &stale)
        guard url.startAccessingSecurityScopedResource() else { throw fail("Folder permission expired; select the folder again") }
        if stale, let updated = try? url.bookmarkData(options: [.minimalBookmark], includingResourceValuesForKeys: nil, relativeTo: nil) { UserDefaults.standard.set(updated, forKey: "MediaManagerFolder-" + key) }
        return url
    }
    nonisolated private static func parts(_ path: String) throws -> [String] {
        if path.isEmpty { return [] }
        let parts = path.components(separatedBy: "/")
        guard parts.allSatisfy({ !$0.isEmpty && $0 != "." && $0 != ".." && !$0.contains("\\") }) else { throw NSError(domain: "Storage", code: 1, userInfo: [NSLocalizedDescriptionKey: "Invalid document path"]) }
        return parts
    }
    nonisolated private static func resolve(_ root: URL, _ path: String) throws -> URL {
        var url = root
        for part in try parts(path) {
            url.appendPathComponent(part)
            let values = try url.resourceValues(forKeys: [.isSymbolicLinkKey])
            if values.isSymbolicLink == true { throw NSError(domain: "Storage", code: 1, userInfo: [NSLocalizedDescriptionKey: "Symbolic links are not supported"]) }
        }
        return url
    }
    private func resolve(_ root: URL, _ path: String) throws -> URL { try Self.resolve(root, path) }
    nonisolated private static func attributes(_ url: URL) throws -> [String: Any] {
        let values = try url.resourceValues(forKeys: [.isDirectoryKey, .fileSizeKey, .contentModificationDateKey, .fileResourceIdentifierKey, .isRegularFileKey])
        guard values.isDirectory == true || values.isRegularFile == true else { throw NSError(domain: "Storage", code: 1, userInfo: [NSLocalizedDescriptionKey: "Unsupported file type"]) }
        return ["dir": values.isDirectory == true, "size": values.fileSize ?? 0,
                "modified": Int64((values.contentModificationDate?.timeIntervalSince1970 ?? 0) * 1000),
                "identity": String(describing: values.fileResourceIdentifier)]
    }
    nonisolated private static func execute(_ command: [String: Any], _ root: URL) throws -> [String: Any] {
        let manager = FileManager.default
        let op = command["op"] as? String ?? ""
        if op == "checkFolder" {
            guard try root.resourceValues(forKeys: [.isDirectoryKey]).isDirectory == true,
                  FileManager.default.isReadableFile(atPath: root.path) else {
                throw NSError(domain: "Storage", code: 1, userInfo: [NSLocalizedDescriptionKey: "Folder access expired. Choose the folder again."])
            }
            return ["connected": true]
        }
        if op == "list" {
            var entries = [[String: Any]]()
            func walk(_ folder: URL, _ prefix: String, _ depth: Int) throws {
                guard depth <= 32 else { throw NSError(domain: "Storage", code: 1, userInfo: [NSLocalizedDescriptionKey: "Folder nesting exceeds 32 levels"]) }
                for child in try manager.contentsOfDirectory(at: folder, includingPropertiesForKeys: [.isSymbolicLinkKey]) {
                    if try child.resourceValues(forKeys: [.isSymbolicLinkKey]).isSymbolicLink == true { continue }
                    let name = child.lastPathComponent; _ = try parts(name)
                    let path = prefix.isEmpty ? name : prefix + "/" + name
                    var entry = try attributes(child); entry["path"] = path; entries.append(entry)
                    guard entries.count <= 20000 else { throw NSError(domain: "Storage", code: 1, userInfo: [NSLocalizedDescriptionKey: "Folder exceeds 20,000 entries"]) }
                    if entry["dir"] as? Bool == true { try walk(child, path, depth + 1) }
                }
            }
            let coordinator = NSFileCoordinator()
            var coordinationError: NSError?; var scanError: Error?
            coordinator.coordinate(readingItemAt: root, options: [], error: &coordinationError) { scoped in do { try walk(scoped, "", 0) } catch { scanError = error } }
            if let error = coordinationError ?? (scanError as NSError?) { throw error }
            return ["name": root.lastPathComponent, "entries": entries, "cache": NSTemporaryDirectory(), "state": try manager.url(for:.applicationSupportDirectory,in:.userDomainMask,appropriateFor:nil,create:true).path]
        }
        let path = command["path"] as? String ?? ""; let pieces = try parts(path)
        guard let last = pieces.last else { throw NSError(domain: "Storage", code: 1) }
        let parent = try resolve(root, pieces.dropLast().joined(separator: "/"))
        let source = parent.appendingPathComponent(last)
        if op == "readMedia" {
            let file = try resolve(root, path)
            let values = try attributes(file)
            let size = (values["size"] as? NSNumber)?.uint64Value ?? 0
            guard let offset = (command["offset"] as? NSNumber)?.uint64Value, let count = command["count"] as? Int,
                  count >= 0, count <= 1048576, offset <= size, UInt64(count) <= size-offset else { throw NSError(domain: "Storage", code: 1, userInfo: [NSLocalizedDescriptionKey: "Invalid media range"]) }
            let handle = try FileHandle(forReadingFrom: file); defer { try? handle.close() }
            try handle.seek(toOffset: offset)
            let data = count == 0 ? Data() : (try handle.read(upToCount: count) ?? Data())
            guard data.count == count else { throw NSError(domain: "Storage", code: 1, userInfo: [NSLocalizedDescriptionKey: "Incomplete media read"]) }
            return ["size": size, "data": data.base64EncodedString()]
        }
        if op == "hash" {
            let before=try attributes(source)
            let file=try FileHandle(forReadingFrom: source); defer {try? file.close()}
            var hash=SHA256()
            while let bytes=try file.read(upToCount:65536), !bytes.isEmpty {hash.update(data:bytes)}
            let after=try attributes(source)
            guard (before["size"] as? NSNumber)==(after["size"] as? NSNumber), (before["modified"] as? NSNumber)==(after["modified"] as? NSNumber) else {throw NSError(domain:"Storage",code:1,userInfo:[NSLocalizedDescriptionKey:"File changed while scanning"])}
            return ["sha256":hash.finalize().map{String(format:"%02x",$0)}.joined()]
        }
        if op == "rename" {
            let name = command["name"] as? String ?? ""
            guard try parts(name).count == 1, !name.contains("/") else { throw NSError(domain: "Storage", code: 1) }
            let destination = parent.appendingPathComponent(name)
            let coordinator = NSFileCoordinator(); var coordinationError: NSError?; var moveError: Error?
            coordinator.coordinate(writingItemAt: source, options: .forMoving, writingItemAt: destination, options: [], error: &coordinationError) { from, to in
                do {
                    _ = try resolve(root, path)
                    let current = try attributes(from)
                    guard current["identity"] as? String == command["identity"] as? String,
                          current["dir"] as? Bool == command["dir"] as? Bool else { throw NSError(domain: "Storage", code: 1, userInfo: [NSLocalizedDescriptionKey: "Document changed; rebuild preview"]) }
                    if current["dir"] as? Bool != true {
                        guard let modified = current["modified"] as? NSNumber, modified.int64Value > 0 else { throw NSError(domain: "Storage", code: 1, userInfo: [NSLocalizedDescriptionKey: "Provider cannot verify file changes safely"]) }
                        guard (current["size"] as? NSNumber) == (command["size"] as? NSNumber), (current["modified"] as? NSNumber) == (command["modified"] as? NSNumber) else { throw NSError(domain: "Storage", code: 1, userInfo: [NSLocalizedDescriptionKey: "File changed since preview"]) }
                    }
                    // moveItem refuses an existing destination; no overwrite fallback.
                    try manager.moveItem(at: from, to: to)
                    coordinator.item(at: from, didMoveTo: to)
                } catch { moveError = error }
            }
            if let error = coordinationError ?? (moveError as NSError?) { throw error }
            return [:]
        }
        if op == "savePoster" {
            guard ["jpg","jpeg","png","webp"].contains(source.pathExtension.lowercased()),
                  let text=command["data"] as? String, let bytes=Data(base64Encoded:text), bytes.count<=10*1024*1024 else {throw NSError(domain:"Storage",code:1)}
            let coordinator=NSFileCoordinator();var coordinationError:NSError?;var saveError:Error?
            coordinator.coordinate(writingItemAt:source,options:[],error:&coordinationError) { destination in
                do {
                    if manager.fileExists(atPath:destination.path) {
                        _ = try resolve(root,path)
                        guard command["replace"] as? Bool == true else {throw NSError(domain:"Storage",code:1,userInfo:[NSLocalizedDescriptionKey:"Confirm Replace existing poster first"])}
                        let attrs=try attributes(destination)
                        guard ((attrs["size"] as? NSNumber)?.intValue ?? Int.max) <= 10*1024*1024 else {throw NSError(domain:"Storage",code:1)}
                        let old=try Data(contentsOf:destination)
                        let hash=SHA256.hash(data:old).map{String(format:"%02x",$0)}.joined()
                        guard hash==command["expected"] as? String else {throw NSError(domain:"Storage",code:1,userInfo:[NSLocalizedDescriptionKey:"Existing poster changed; select it again"])}
                        try bytes.write(to:destination,options:[.atomic])
                    } else {try bytes.write(to:destination,options:[.withoutOverwriting])}
                } catch {saveError=error}
            }
            if let error=coordinationError ?? (saveError as NSError?) {throw error}
            return [:]
        }
        if op == "write" {
            guard let text = command["data"] as? String, let bytes = Data(base64Encoded: text), bytes.count <= 10 * 1024 * 1024 else { throw NSError(domain: "Storage", code: 1) }
            if manager.fileExists(atPath: source.path) { return ["kept": true] }
            let coordinator = NSFileCoordinator(); var coordinationError: NSError?; var writeError: Error?
            coordinator.coordinate(writingItemAt: source, options: [], error: &coordinationError) { destination in
                do { try bytes.write(to: destination, options: [.withoutOverwriting]) } catch { writeError = error }
            }
            if let error = coordinationError ?? (writeError as NSError?) { throw error }
            return [:]
        }
        throw NSError(domain: "Storage", code: 1, userInfo: [NSLocalizedDescriptionKey: "Unknown storage command"])
    }
    public func previewControllerDidDismiss(_ controller: QLPreviewController) {
        previewRoot?.stopAccessingSecurityScopedResource(); previewRoot = nil; previewURL = nil
    }
    public func numberOfPreviewItems(in controller: QLPreviewController) -> Int { previewURL == nil ? 0 : 1 }
    public func previewController(_ controller: QLPreviewController, previewItemAt index: Int) -> QLPreviewItem { previewURL! as NSURL }
}
