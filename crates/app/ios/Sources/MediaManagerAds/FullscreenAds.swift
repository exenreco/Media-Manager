import UIKit
import GoogleMobileAds

/// Device-local ad-free time is granted only by Google's earned-reward callback.
@MainActor
final class FullscreenAds: NSObject, FullScreenContentDelegate {
    private weak var controller: UIViewController?
    private let allowed: () -> Bool
    private let publish: ([String: Any]) -> Void
    private let changed: () -> Void
    private let prefs = UserDefaults.standard
    private let sessionStart = ProcessInfo.processInfo.systemUptime
    private var interstitial: InterstitialAd?
    private var rewarded: RewardedAd?
    private var interstitialAt: TimeInterval = 0
    private var rewardedAt: TimeInterval = 0
    private var loadingInterstitial = false
    private var loadingRewarded = false
    private var generation = 0
    private var disposed = false
    private var expiry: Timer?
    private var showingAd: AnyObject?
    private var earned = false
    private var showingReward = false
    private(set) var presenting = false
    var adFree: Bool { prefs.double(forKey: "mediaManagerAdFreeUntil") > Date().timeIntervalSince1970 }
    var suppressed: Bool { adFree || presenting }

    init(controller: UIViewController?, allowed: @escaping () -> Bool,
         publish: @escaping ([String: Any]) -> Void, changed: @escaping () -> Void) {
        self.controller = controller; self.allowed = allowed; self.publish = publish; self.changed = changed
        super.init()
    }
    func state(_ message: String = "") {
        guard !disposed else { return }
        let until = prefs.double(forKey: "mediaManagerAdFreeUntil")
        publish(["adFreeUntil": until * 1000, "presenting": presenting,
                 "rewardedReady": rewarded != nil && fresh(rewardedAt), "message": message])
        expiry?.invalidate(); expiry = nil
        if adFree {
            expiry = Timer.scheduledTimer(withTimeInterval: max(0.1, until - Date().timeIntervalSince1970 + 0.05), repeats: false) { [weak self] _ in
                Task { @MainActor [weak self] in self?.state(); self?.changed(); self?.load() }
            }
        }
    }
    private func fresh(_ time: TimeInterval) -> Bool { ProcessInfo.processInfo.systemUptime - time < 3600 }
    func reset() { generation += 1; interstitial = nil; rewarded = nil; loadingInterstitial = false; loadingRewarded = false; state() }
    func close() { disposed = true; reset(); expiry?.invalidate(); expiry = nil }
    func load() {
        guard !disposed, allowed(), !suppressed else { return }
        if !fresh(interstitialAt) { interstitial = nil }
        if !fresh(rewardedAt) { rewarded = nil }
        let epoch = generation
        #if DEBUG
        let interstitialUnit = "ca-app-pub-3940256099942544/4411468910"
        let rewardedUnit = "ca-app-pub-3940256099942544/1712485313"
        #else
        let interstitialUnit = "ca-app-pub-9696662815161119/7074143339"
        let rewardedUnit = "ca-app-pub-9696662815161119/8079057063"
        #endif
        if interstitial == nil && !loadingInterstitial {
            loadingInterstitial = true
            Task { @MainActor [weak self] in
                do {
                    let ad = try await InterstitialAd.load(with: interstitialUnit, request: Request())
                    guard let self, !self.disposed, self.generation == epoch else { return }
                    self.loadingInterstitial = false
                    if self.allowed() && !self.adFree { self.interstitial = ad; self.interstitialAt = ProcessInfo.processInfo.systemUptime; ad.fullScreenContentDelegate = self }
                    self.state()
                } catch {
                    guard let self, !self.disposed, self.generation == epoch else { return }
                    self.loadingInterstitial = false; self.state()
                }
            }
        }
        if rewarded == nil && !loadingRewarded {
            loadingRewarded = true
            Task { @MainActor [weak self] in
                do {
                    let ad = try await RewardedAd.load(with: rewardedUnit, request: Request())
                    guard let self, !self.disposed, self.generation == epoch else { return }
                    self.loadingRewarded = false
                    if self.allowed() && !self.adFree { self.rewarded = ad; self.rewardedAt = ProcessInfo.processInfo.systemUptime; ad.fullScreenContentDelegate = self }
                    self.state()
                } catch {
                    guard let self, !self.disposed, self.generation == epoch else { return }
                    self.loadingRewarded = false; self.state("No rewarded ad is available. Please try again later.")
                }
            }
        }
    }
    func show(reward: Bool) {
        guard !disposed, allowed(), !suppressed, let controller,
              controller.viewIfLoaded?.window != nil, controller.presentedViewController == nil else { return }
        if !reward && (ProcessInfo.processInfo.systemUptime - sessionStart < 120 ||
            Date().timeIntervalSince1970 - prefs.double(forKey: "mediaManagerLastInterstitial") < 600) { return }
        if reward && (rewarded == nil || !fresh(rewardedAt)) {
            rewarded = nil; load(); state("Rewarded ad not ready. Please try again shortly."); return
        }
        if !reward && (interstitial == nil || !fresh(interstitialAt)) { interstitial = nil; load(); return }
        presenting = true; showingReward = reward; earned = false; changed(); state()
        if reward, let ad = rewarded {
            rewarded = nil; showingAd = ad
            ad.present(from: controller) { [weak self] in
                guard let self, !self.disposed, !self.earned else { return }
                self.earned = true
                self.prefs.set(Date().timeIntervalSince1970 + 1800, forKey: "mediaManagerAdFreeUntil")
                self.interstitial = nil; self.state(); self.changed()
            }
        } else if let ad = interstitial { interstitial = nil; showingAd = ad; ad.present(from: controller) }
    }
    func adWillPresentFullScreenContent(_ ad: FullScreenPresentingAd) {
        if !showingReward { prefs.set(Date().timeIntervalSince1970, forKey: "mediaManagerLastInterstitial") }
    }
    func adDidDismissFullScreenContent(_ ad: FullScreenPresentingAd) {
        finish(showingReward && !earned ? "Ad closed. No reward was earned." : "")
    }
    func ad(_ ad: FullScreenPresentingAd, didFailToPresentFullScreenContentWithError error: Error) {
        finish("Ad could not be shown. Please try again later.")
    }
    private func finish(_ message: String) { showingAd = nil; presenting = false; state(message); changed(); load() }
}
