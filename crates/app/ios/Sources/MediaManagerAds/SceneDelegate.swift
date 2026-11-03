import UIKit

/// Hosts Dioxus/Tao's single existing window in UIKit's scene lifecycle.
/// Tao 0.34 still emits its lifecycle events from UIApplicationDelegate methods.
@MainActor
@objc(MediaManagerSceneDelegate)
final class MediaManagerSceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?
    private static weak var active: MediaManagerSceneDelegate?
    private weak var connectedScene: UIWindowScene?

    static func adopt(_ window: UIWindow?) {
        guard let owner = active, let scene = owner.connectedScene, let window,
              window.windowScene !== scene else { return }
        owner.window = window
        window.windowScene = scene
        window.transform = .identity
        window.frame = scene.coordinateSpace.bounds
        window.bounds = CGRect(origin: .zero, size: scene.coordinateSpace.bounds.size)
        if let root = window.rootViewController?.view {
            root.transform = .identity
            root.frame = window.bounds
            root.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        }
        window.makeKeyAndVisible()
        window.layoutIfNeeded()
        NSLog("MediaManagerScene adopted window frame=%@ bounds=%@ root=%@", NSCoder.string(for: window.frame), NSCoder.string(for: window.bounds), NSCoder.string(for: window.rootViewController?.view.frame ?? .zero))
    }

    func scene(_ scene: UIScene, willConnectTo session: UISceneSession,
               options connectionOptions: UIScene.ConnectionOptions) {
        guard let windowScene = scene as? UIWindowScene else { return }
        Self.active = self
        connectedScene = windowScene
        // Dioxus creates its window on the next run-loop turn. It calls adopt
        // when attaching its native bridge, after the root controller exists.
        Self.adopt(UIApplication.shared.windows.first(where: {
            NSStringFromClass(type(of: $0)).contains("TaoUIWindow")
        }))
    }

    func sceneDidBecomeActive(_ scene: UIScene) {
        UIApplication.shared.delegate?.applicationDidBecomeActive?(UIApplication.shared)
    }

    func sceneWillResignActive(_ scene: UIScene) {
        UIApplication.shared.delegate?.applicationWillResignActive?(UIApplication.shared)
    }

    func sceneWillEnterForeground(_ scene: UIScene) {
        UIApplication.shared.delegate?.applicationWillEnterForeground?(UIApplication.shared)
    }

    func sceneDidEnterBackground(_ scene: UIScene) {
        UIApplication.shared.delegate?.applicationDidEnterBackground?(UIApplication.shared)
    }
}
