package dev.dioxus.main

import org.junit.Assert.*
import org.junit.Test

class PlayEntitlementSyncTest {
    @Test fun inFlightQueryCannotUndoPurchaseCallback() {
        val sync = PlayEntitlementSync()
        val beforePurchase = sync.revision
        sync.purchaseEvent()
        assertFalse(sync.acceptsQuery(beforePurchase))
        assertTrue(sync.acceptsQuery(sync.revision))
        val beforePendingEvent = sync.revision
        sync.purchaseEvent()
        assertFalse(sync.acceptsQuery(beforePendingEvent))
    }

    @Test fun samePurchaseCannotRegressFromVerifiedPurchasedToPending() {
        val sync = PlayEntitlementSync()
        assertTrue(sync.isPendingRegression("verified-token", "verified-token", true))
        assertFalse(sync.isPendingRegression("verified-token", "different-token", true))
        assertFalse(sync.isPendingRegression("", "", true))
        // Empty ownership snapshots can still revoke ownership; this guard only
        // handles the impossible PURCHASED -> PENDING transition of one token.
        assertFalse(sync.isPendingRegression("verified-token", "verified-token", false))
    }
}
