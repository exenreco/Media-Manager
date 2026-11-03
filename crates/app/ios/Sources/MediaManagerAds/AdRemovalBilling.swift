import Foundation
import StoreKit

/// StoreKit owns payment UI, transaction verification and durable entitlements.
/// Neither WebKit messages nor a saved preference can grant ad removal.
@MainActor
final class AdRemovalBilling {
    private let productID: String
    private let publish: ([String: Any]) -> Void
    private let changed: () -> Void
    private var product: Product?
    private var listener: Task<Void, Never>?
    private var operation: Task<Void, Never>?
    private var started = false
    private var disposed = false
    private var checked = false
    private var busy = false
    private var pending = false
    private var revision = 0
    private var restoreUntil: TimeInterval = 0
    private var canRestore: Bool { owned && Date().timeIntervalSince1970 < restoreUntil }
    private var message = "Checking the App Store…"
    private(set) var owned = false
    private(set) var presenting = false
    var suppressesAds: Bool { !checked || owned || presenting }

    init(productID: String = Bundle.main.object(forInfoDictionaryKey: "MediaManagerRemoveAdsProductID") as? String ?? "remove_ads",
         publish: @escaping ([String: Any]) -> Void, changed: @escaping () -> Void) {
        self.productID = productID
        self.publish = publish
        self.changed = changed
    }

    func start() {
        guard !started, !disposed else { state(); return }
        started = true
        listener = Task { @MainActor [weak self] in
            for await result in Transaction.updates {
                guard let self, !self.disposed, !Task.isCancelled else { return }
                guard case .verified(let transaction) = result,
                      transaction.productID == self.productID,
                      transaction.productType == .nonConsumable else { continue }
                self.revision += 1
                self.owned = transaction.revocationDate == nil
                self.pending = false
                self.changed()
                await self.readEntitlements()
                await transaction.finish()
                self.message = self.owned ? "All ads removed on iOS." : "The Remove ads purchase is no longer active."
                self.state()
            }
        }
        refresh()
    }

    func state() {
        guard !disposed else { return }
        publish(["owned": owned, "busy": busy || !checked, "pending": pending,
                 "available": product != nil, "price": product?.displayPrice ?? "",
                 "presenting": presenting, "restoreUntil": restoreUntil * 1000,
                 "canRestore": canRestore, "message": message])
    }

    /// StoreKit keeps its signed transaction cache available offline. Never trust a
    /// cached Boolean; recount verified, unrevoked non-consumable entitlements.
    private func readEntitlements() async {
        while !disposed && !Task.isCancelled {
            let epoch = revision
            var entitled = false
            var deadline: TimeInterval = 0
            for await result in Transaction.currentEntitlements {
                if case .verified(let transaction) = result,
                   transaction.productID == productID,
                   transaction.productType == .nonConsumable,
                   transaction.revocationDate == nil {
                    entitled = true
                    deadline = max(deadline, transaction.purchaseDate.timeIntervalSince1970 + 600)
                }
            }
            guard !disposed, !Task.isCancelled else { return }
            // A purchase/update received during this scan takes precedence.
            if epoch != revision { continue }
            owned = entitled
            restoreUntil = deadline
            checked = true
            if owned { pending = false }
            changed()
            return
        }
    }

    func refresh(restore: Bool = false) {
        guard !disposed, !busy else { state(); return }
        if restore && !canRestore {
            message = owned
                ? "The 10-minute restore window has ended. Purchase status still updates automatically."
                : "Restore purchases is unavailable because no verified purchase was found."
            state()
            return
        }
        busy = true
        message = restore ? "Restoring purchases…" : "Checking the App Store…"
        state()
        operation = Task { @MainActor [weak self] in
            guard let self else { return }
            defer { self.complete() }
            do {
                // Only an explicit Restore tap may trigger Apple's account UI.
                if restore {
                    self.presenting = true; self.changed(); self.state()
                    try await AppStore.sync()
                    self.presenting = false
                }
                await self.readEntitlements()
                guard !self.disposed, !Task.isCancelled else { return }
                if self.owned {
                    self.message = "All ads removed on iOS."
                    return
                }
                self.product = try await Product.products(for: [self.productID])
                    .first(where: { $0.id == self.productID && $0.type == .nonConsumable })
                self.message = self.product == nil
                    ? "Remove ads is unavailable. Try again from an App Store or TestFlight build."
                    : restore ? "No Remove ads purchase was found for this Apple Account." : "One-time purchase. No subscription."
            } catch {
                self.message = restore ? "Could not restore purchases. Please try again." : "Could not load the App Store price. Please try again later."
            }
        }
    }

    func buy() {
        guard !disposed, checked, !busy, !owned, !pending, product != nil else { state(); return }
        busy = true; presenting = true
        message = "Opening the App Store…"
        changed(); state()
        operation = Task { @MainActor [weak self] in
            guard let self else { return }
            defer { self.complete() }
            do {
                // Refresh the product immediately before opening Apple's checkout.
                guard let product = try await Product.products(for: [self.productID])
                    .first(where: { $0.id == self.productID && $0.type == .nonConsumable }),
                      !self.disposed, !Task.isCancelled else {
                    self.product = nil
                    self.message = "Remove ads is currently unavailable. Please try again later."
                    return
                }
                self.product = product
                let result = try await product.purchase()
                guard !self.disposed, !Task.isCancelled else { return }
                switch result {
                case .success(let result):
                    guard case .verified(let transaction) = result,
                          transaction.productID == self.productID,
                          transaction.productType == .nonConsumable else {
                        self.message = "Apple could not verify this purchase. Purchase status will update automatically."
                        return
                    }
                    self.revision += 1
                    self.owned = transaction.revocationDate == nil
                    self.pending = false
                    self.changed()
                    await self.readEntitlements()
                    await transaction.finish()
                    self.message = self.owned ? "All ads removed on iOS." : "This purchase is no longer active."
                case .pending:
                    self.pending = true
                    self.message = "Payment is pending approval. Ads will be removed after Apple confirms the purchase."
                case .userCancelled:
                    self.message = "Purchase cancelled."
                @unknown default:
                    self.message = "Purchase not completed. Please try again."
                }
            } catch {
                self.message = "The purchase could not be completed. Please try again."
            }
        }
    }

    private func complete() {
        busy = false; presenting = false; operation = nil
        guard !disposed else { return }
        changed(); state()
    }

    func close() {
        disposed = true
        operation?.cancel(); operation = nil
        listener?.cancel(); listener = nil
    }
}
