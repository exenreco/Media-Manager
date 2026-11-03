import Foundation
import StoreKit

private let productID = "desktop_pro"
typealias ProCallback = @convention(c) (UInt64, UnsafePointer<CChar>?) -> Void

// All grants come from verified StoreKit transactions, including offline cache.
// No preference, browser callback, or cached Rust Boolean can grant ownership.
@MainActor
private func owned() async -> Bool {
    for await result in Transaction.currentEntitlements {
        if case .verified(let transaction) = result,
           transaction.productID == productID,
           transaction.productType == .nonConsumable,
           transaction.revocationDate == nil {
            return true
        }
    }
    return false
}

@_cdecl("media_manager_pro_request")
func requestPro(_ action: Int32, _ requestID: UInt64, _ callback: @escaping ProCallback) {
    Task { @MainActor in
        var message = ""
        var price = ""
        var available = false
        do {
            if action == 2 { try await AppStore.sync() }
            let products = try await Product.products(for: [productID])
            let product = products.first(where: { $0.id == productID && $0.type == .nonConsumable })
            available = product != nil
            price = product?.displayPrice ?? ""
            if action == 1, let product {
                switch try await product.purchase() {
                case .success(let result):
                    guard case .verified(let transaction) = result,
                          transaction.productID == productID,
                          transaction.productType == .nonConsumable,
                          transaction.revocationDate == nil else {
                        throw NSError(domain: "MediaManagerPro", code: 1,
                                      userInfo: [NSLocalizedDescriptionKey: "The App Store purchase could not be verified."])
                    }
                    await transaction.finish()
                    message = "Pro purchase verified."
                case .pending: message = "Purchase pending approval. Pro unlocks after verification."
                case .userCancelled: message = "Purchase cancelled."
                @unknown default: message = "The App Store did not complete the purchase."
                }
            } else if action == 1 {
                message = "Pro is not available from the App Store for this build yet."
            }
            for await result in Transaction.unfinished {
                if case .verified(let transaction) = result, transaction.productID == productID {
                    await transaction.finish()
                }
            }
        } catch {
            message = "App Store: \(error.localizedDescription)"
        }
        let isOwned = await owned()
        if message.isEmpty {
            message = isOwned ? "Pro is active. Unlimited movie and TV show titles."
                : available ? "One-time purchase. Unlimited movie and TV show titles."
                : "Pro is not available from the App Store for this build yet."
        }
        let state: [String: Any] = ["owned": isOwned, "available": available,
                                    "price": price, "message": message]
        let data = try! JSONSerialization.data(withJSONObject: state)
        String(decoding: data, as: UTF8.self).withCString { callback(requestID, $0) }
    }
}
