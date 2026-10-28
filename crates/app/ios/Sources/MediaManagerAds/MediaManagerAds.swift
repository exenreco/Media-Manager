import UIKit
import WebKit
import GoogleMobileAds
import UserMessagingPlatform

/// Only trusted layout, privacy and fullscreen commands cross WebKit. The SDK owns ad clicks.
@MainActor
@objc(MediaManagerAds)
public final class MediaManagerAds: NSObject, WKScriptMessageHandler, NativeAdLoaderDelegate {
    private weak var controller: UIViewController?
    private weak var browser: WKWebView?
    private var loader: AdLoader?
    private var ad: NativeAd?
    private var adView: NativeAdView?
    private var displayLink: CADisplayLink?
    private var expiry: Timer?
    private var geometry: [String: Any]?
    private var started = false
    private var disposed = false
    private var loading = false
    private var consentInFlight = false
    private var initializing = false
    private static var sdkReady = false
    private lazy var fullscreen: FullscreenAds = FullscreenAds(controller: controller,
        allowed: { [weak self] in self?.disposed == false && Self.sdkReady && ConsentInformation.shared.canRequestAds && UIApplication.shared.applicationState == .active },
        publish: { [weak self] value in
            var payload = value
            payload["privacyRequired"] = ConsentInformation.shared.privacyOptionsRequirementStatus == .required
            guard let bytes = try? JSONSerialization.data(withJSONObject: payload), let json = String(data: bytes, encoding: .utf8) else { return }
            self?.browser?.evaluateJavaScript("window.mediaManagerFullscreenState?.(\(json))")
        }, changed: { [weak self] in self?.positionAd(); if self?.fullscreen.suppressed == false { self?.loadAd() } })

    public override init() { super.init() }

    @objc public func attach(_ address: UInt64) {
        guard let pointer = UnsafeRawPointer(bitPattern: UInt(address)) else { return }
        let host = Unmanaged<UIViewController>.fromOpaque(pointer).takeUnretainedValue()
        controller = host
        disposed = false
        attachBrowser(remaining: 60)
    }

    private func attachBrowser(remaining: Int) {
        guard !disposed, browser == nil, let controller else { return }
        guard let web = findWebView(controller.view) else {
            if remaining > 0 {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [weak self] in self?.attachBrowser(remaining: remaining - 1) }
            }
            return
        }
        browser = web
        web.configuration.userContentController.add(self, name: "MediaManagerAds")
        displayLink = CADisplayLink(target: self, selector: #selector(positionAd))
        displayLink?.add(to: .main, forMode: .common)
        NotificationCenter.default.addObserver(self, selector: #selector(hide),
            name: UIApplication.willResignActiveNotification, object: nil)
    }

    @objc public func detach() {
        disposed = true
        fullscreen.close()
        browser?.configuration.userContentController.removeScriptMessageHandler(forName: "MediaManagerAds")
        displayLink?.invalidate(); displayLink = nil
        NotificationCenter.default.removeObserver(self)
        discardAd()
        loader?.delegate = nil; loader = nil
        browser = nil; controller = nil; geometry = nil
    }

    private func findWebView(_ view: UIView) -> WKWebView? {
        if let web = view as? WKWebView { return web }
        for child in view.subviews { if let web = findWebView(child) { return web } }
        return nil
    }

    public func userContentController(_ userContentController: WKUserContentController,
                                     didReceive message: WKScriptMessage) {
        guard !disposed, message.frameInfo.isMainFrame,
              let url = message.frameInfo.request.url,
              url.scheme == "dioxus", url.host == "index.html",
              let text = message.body as? String, let data = text.data(using: .utf8),
              let command = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
        else { return }
        switch command["type"] as? String {
        case "fullscreen-state":
            fullscreen.state(); fullscreen.load()
            if !started { started = true; requestConsent() }
        case "interstitial": fullscreen.show(reward: false)
        case "rewarded":
            fullscreen.show(reward: true)
            if !ConsentInformation.shared.canRequestAds { requestConsent() }
        case "bounds":
            geometry = command
            positionAd()
            if !started && command["eligible"] as? Bool == true {
                started = true; requestConsent()
            }
        case "retry":
            if !loading && !consentInFlight && geometry?["eligible"] as? Bool == true {
                started = true; requestConsent()
            }
        case "hide": geometry = nil; hide()
        case "privacy": presentPrivacyOptions()
        default: break
        }
    }

    private func requestConsent() {
        guard !disposed, !consentInFlight else { return }
        consentInFlight = true
        report("Checking advertising privacy choices…")
        ConsentInformation.shared.requestConsentInfoUpdate(with: RequestParameters()) { [weak self] error in
            guard let self, !self.disposed, let controller = self.controller else { return }
            self.updatePrivacyButton()
            guard error == nil else {
                self.consentInFlight = false
                if ConsentInformation.shared.canRequestAds { self.initializeAds() }
                else { self.collapse("Could not check advertising privacy choices. Please retry.") }
                return
            }
            ConsentForm.loadAndPresentIfRequired(from: controller) { [weak self] error in
                guard let self, !self.disposed else { return }
                self.consentInFlight = false
                self.updatePrivacyButton()
                if ConsentInformation.shared.canRequestAds { self.initializeAds() }
                else { self.collapse() }
            }
        }
    }

    private func presentPrivacyOptions() {
        guard ConsentInformation.shared.privacyOptionsRequirementStatus == .required,
              let controller, !disposed else { return }
        loader?.delegate = nil; loader = nil; loading = false
        discardAd(); fullscreen.reset()
        ConsentForm.presentPrivacyOptionsForm(from: controller) { [weak self] error in
            guard let self, !self.disposed else { return }
            self.updatePrivacyButton()
            if error == nil && ConsentInformation.shared.canRequestAds { self.initializeAds() }
            else { self.collapse() }
        }
    }

    private func updatePrivacyButton() {
        fullscreen.state()
        let required = ConsentInformation.shared.privacyOptionsRequirementStatus == .required
        browser?.evaluateJavaScript("document.querySelectorAll('.ad-privacy').forEach(button => button.toggleAttribute('hidden', \(!required)))")
    }

    private func initializeAds() {
        guard !disposed, ConsentInformation.shared.canRequestAds else { return }
        if Self.sdkReady { loadAd(); fullscreen.load(); return }
        guard !initializing else { return }; initializing = true
        report("Starting advertising…")
        MobileAds.shared.start { [weak self] _ in
            Self.sdkReady = true
            self?.initializing = false
            self?.loadAd(); self?.fullscreen.load()
        }
    }

    private func loadAd() {
        guard !disposed, !fullscreen.suppressed, !loading, ad == nil, let controller, ConsentInformation.shared.canRequestAds else { return }
        loading = true
        report("Loading advertisement…")
        #if DEBUG
        let unit = "ca-app-pub-3940256099942544/3986624511"
        #else
        let unit = "ca-app-pub-9696662815161119/9079018827"
        #endif
        let options = NativeAdViewAdOptions()
        options.preferredAdChoicesPosition = .topRightCorner
        loader = AdLoader(adUnitID: unit, rootViewController: controller, adTypes: [.native], options: [options])
        loader?.delegate = self
        loader?.load(Request())
    }

    public func adLoader(_ adLoader: AdLoader, didReceive nativeAd: NativeAd) {
        guard adLoader === loader else { return }
        loading = false
        guard !disposed, let browser, browser.window != nil else { return }
        discardAd()
        ad = nativeAd
        let view = makeView(nativeAd)
        adView = view
        browser.window?.addSubview(view)
        browser.evaluateJavaScript("window.mediaManagerAdSetHidden?.(false)")
        expiry = Timer.scheduledTimer(withTimeInterval: 3600, repeats: false) { [weak self] _ in
            Task { @MainActor [weak self] in self?.discardAd(); self?.collapse() }
        }
        report("")
        positionAd()
    }

    public func adLoader(_ adLoader: AdLoader, didFailToReceiveAdWithError error: Error) {
        guard adLoader === loader else { return }
        loading = false
        let detail = error as NSError
        NSLog("MediaManagerAds: %@", detail.description)
        collapse("Ad unavailable (code \(detail.code)). You can retry.")
    }

    private func makeView(_ ad: NativeAd) -> NativeAdView {
        let view = NativeAdView(frame: .zero)
        view.isHidden = true
        view.backgroundColor = UIColor(red: 30/255, green: 43/255, blue: 59/255, alpha: 1)
        let stack = UIStackView()
        stack.axis = .vertical; stack.spacing = 2
        stack.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 12),
            stack.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -12),
            stack.topAnchor.constraint(equalTo: view.topAnchor, constant: 12),
            stack.bottomAnchor.constraint(equalTo: view.bottomAnchor, constant: -12)
        ])
        func label(_ text: String?, height: CGFloat, lines: Int = 2) -> UILabel {
            let label = UILabel(); label.text = text; label.textColor = .white
            label.font = .systemFont(ofSize: 12); label.numberOfLines = lines
            label.heightAnchor.constraint(equalToConstant: height).isActive = true
            stack.addArrangedSubview(label)
            return label
        }
        let badge = label("Advertisement", height: 24, lines: 1)
        badge.isUserInteractionEnabled = false
        let heading = UIStackView(); heading.axis = .horizontal; heading.spacing = 8
        heading.heightAnchor.constraint(equalToConstant: 52).isActive = true
        stack.addArrangedSubview(heading)
        let icon = UIImageView(image: ad.icon?.image); icon.contentMode = .scaleAspectFit
        icon.widthAnchor.constraint(equalToConstant: 44).isActive = true
        icon.isHidden = ad.icon == nil; heading.addArrangedSubview(icon)
        let headline = UILabel(); headline.text = ad.headline; headline.textColor = .white
        headline.font = .boldSystemFont(ofSize: 16); headline.numberOfLines = 2
        heading.addArrangedSubview(headline)
        let advertiser = label(ad.advertiser, height: 20); advertiser.isHidden = ad.advertiser == nil
        let body = label(ad.body, height: 36, lines: 2); body.isHidden = ad.body == nil
        let media = MediaView(); media.mediaContent = ad.mediaContent
        media.heightAnchor.constraint(greaterThanOrEqualToConstant: 120).isActive = true
        stack.addArrangedSubview(media)
        let action = UIButton(type: .system); action.setTitle(ad.callToAction, for: .normal)
        action.backgroundColor = UIColor(red: 157/255, green: 232/255, blue: 208/255, alpha: 1)
        action.setTitleColor(.black, for: .normal)
        action.heightAnchor.constraint(equalToConstant: 48).isActive = true
        action.isHidden = ad.callToAction == nil
        action.isUserInteractionEnabled = false // The SDK, not this button, handles ad clicks.
        stack.addArrangedSubview(action)
        view.headlineView = headline; view.iconView = icon; view.advertiserView = advertiser
        view.bodyView = body; view.mediaView = media; view.callToActionView = action
        view.nativeAd = ad
        return view
    }

    @objc private func positionAd() {
        guard let view = adView else { return }
        guard !disposed, !fullscreen.suppressed, UIApplication.shared.applicationState == .active,
              controller?.presentedViewController == nil,
              let browser, let window = browser.window, let geometry,
              geometry["visible"] as? Bool == true,
              let viewport = geometry["viewportWidth"] as? Double,
              viewport.isFinite && viewport > 0 else { hide(); return }
        let names = ["x", "y", "width", "height"]
        let values = names.compactMap { geometry[$0] as? Double }
        guard values.count == 4, values.allSatisfy({ $0.isFinite && $0 >= 0 && $0 < 100_000 })
        else { hide(); return }
        let scale = browser.bounds.width / CGFloat(viewport)
        let rect = CGRect(x: CGFloat(values[0]) * scale, y: CGFloat(values[1]) * scale,
                          width: CGFloat(values[2]) * scale, height: CGFloat(values[3]) * scale)
        guard rect.width >= 239, rect.height >= 379, browser.bounds.insetBy(dx: -1, dy: -1).contains(rect)
        else { hide(); return }
        let compact = rect.height < 440
        view.advertiserView?.isHidden = compact || ad?.advertiser == nil
        view.bodyView?.isHidden = compact || ad?.body == nil
        view.frame = browser.convert(rect, to: window)
        view.isHidden = false
    }

    @objc private func hide() { adView?.isHidden = true }
    private func report(_ message: String, retry: Bool = false) {
        guard let bytes = try? JSONSerialization.data(withJSONObject: [message]),
              let array = String(data: bytes, encoding: .utf8) else { return }
        browser?.evaluateJavaScript("window.mediaManagerAdStatus?.(\(array)[0], \(retry))")
    }
    private func collapse(_ message: String = "No advertisement is available. You can retry.") {
        report(message, retry: true)
        hide()
        browser?.evaluateJavaScript("window.mediaManagerAdSetHidden?.(true)")
    }
    private func discardAd() {
        expiry?.invalidate(); expiry = nil
        adView?.removeFromSuperview(); adView?.nativeAd = nil; adView = nil; ad = nil
    }
}
