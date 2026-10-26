import UIKit
import AVFoundation

/// Camera frames stay on-device. Only the decoded QR text reaches the app bridge.
@MainActor
final class PairingCameraController: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
    private let session = AVCaptureSession()
    private let queue = DispatchQueue(label: "MediaManager.PairingCamera")
    private var preview: AVCaptureVideoPreviewLayer?
    private let guide = UIView()
    private var completed = false
    private let completion: (Result<String, Error>) -> Void
    init(completion: @escaping (Result<String, Error>) -> Void) {
        self.completion = completion
        super.init(nibName: nil, bundle: nil)
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) is unavailable") }
    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        let close = UIButton(type: .system)
        close.setTitle("Cancel", for: .normal)
        close.tintColor = .white
        close.addTarget(self, action: #selector(cancel), for: .touchUpInside)
        let label = UILabel()
        label.text = "Align the desktop Media Manager QR code inside the guide"
        label.textColor = .white; label.numberOfLines = 0; label.textAlignment = .center
        guide.layer.borderWidth = 3; guide.layer.borderColor = UIColor.systemMint.cgColor
        guide.layer.cornerRadius = 16; guide.isUserInteractionEnabled = false
        for item in [guide, close, label] { item.translatesAutoresizingMaskIntoConstraints = false; view.addSubview(item) }
        NSLayoutConstraint.activate([
            guide.centerXAnchor.constraint(equalTo: view.centerXAnchor), guide.centerYAnchor.constraint(equalTo: view.centerYAnchor),
            guide.widthAnchor.constraint(equalTo: view.widthAnchor, multiplier: 0.72), guide.heightAnchor.constraint(equalTo: guide.widthAnchor),
            close.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 12), close.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 24),
            label.topAnchor.constraint(equalTo: guide.bottomAnchor, constant: 24), label.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 24), label.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -24)
        ])
        do {
            guard let camera = AVCaptureDevice.default(for: .video) else { throw scannerError("No camera available on this device") }
            let input = try AVCaptureDeviceInput(device: camera)
            let output = AVCaptureMetadataOutput()
            guard session.canAddInput(input), session.canAddOutput(output) else { throw scannerError("Camera scanner is unavailable") }
            session.addInput(input); session.addOutput(output)
            output.setMetadataObjectsDelegate(self, queue: .main); output.metadataObjectTypes = [.qr]
            let layer = AVCaptureVideoPreviewLayer(session: session); layer.videoGravity = .resizeAspectFill
            view.layer.insertSublayer(layer, at: 0); preview = layer
        } catch { DispatchQueue.main.async { self.finish(.failure(error)) } }
    }
    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        guard !completed, preview != nil else { return }
        let capture = session; queue.async { capture.startRunning() }
        NotificationCenter.default.addObserver(self, selector: #selector(backgrounded), name: UIApplication.didEnterBackgroundNotification, object: nil)
    }
    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        NotificationCenter.default.removeObserver(self)
        let capture = session; queue.async { capture.stopRunning() }
    }
    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews(); preview?.frame = view.bounds
        if let connection = preview?.connection, connection.isVideoOrientationSupported {
            switch view.window?.windowScene?.interfaceOrientation {
            case .landscapeLeft: connection.videoOrientation = .landscapeLeft
            case .landscapeRight: connection.videoOrientation = .landscapeRight
            case .portraitUpsideDown: connection.videoOrientation = .portraitUpsideDown
            default: connection.videoOrientation = .portrait
            }
        }
    }
    func metadataOutput(_ output: AVCaptureMetadataOutput, didOutput objects: [AVMetadataObject], from connection: AVCaptureConnection) {
        guard !completed, let preview else { return }
        for case let code as AVMetadataMachineReadableCodeObject in objects {
            guard let text = code.stringValue, let visible = preview.transformedMetadataObject(for: code), guide.frame.contains(visible.bounds) else { continue }
            guard text.utf8.count <= 8192 else { finish(.failure(scannerError("Pairing QR is too large"))); return }
            finish(.success(text)); return
        }
    }
    @objc private func cancel() { finish(.failure(scannerError("Scanning cancelled"))) }
    @objc private func backgrounded() { cancel() }
    private func scannerError(_ message: String) -> NSError { NSError(domain: "MediaManagerPairing", code: 1, userInfo: [NSLocalizedDescriptionKey: message]) }
    private func finish(_ result: Result<String, Error>) {
        guard !completed else { return }; completed = true
        let capture = session; queue.async { capture.stopRunning() }
        dismiss(animated: true) { self.completion(result) }
    }
}
