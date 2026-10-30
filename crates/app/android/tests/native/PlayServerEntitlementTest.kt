package dev.dioxus.main

import org.junit.Assert.*
import org.junit.Test

class PlayServerEntitlementTest {
    @Test fun onlyAcknowledgedCompletedPurchaseGrantsOwnership() {
        assertTrue(PlayServerEntitlement.parse(true, "PURCHASED", true).owned)
        assertTrue(PlayServerEntitlement.parse(false, "PENDING", false).pending)
        for (state in listOf("CANCELLED", "NOT_OWNED")) {
            assertFalse(PlayServerEntitlement.parse(false, state, false).owned)
            assertFalse(PlayServerEntitlement.parse(false, state, false).pending)
        }
    }
    @Test fun malformedAndContradictoryResponsesAreTransientFailures() {
        for ((owned, state, ack) in listOf(Triple(true,"PENDING",true), Triple(false,"PURCHASED",true),
            Triple(true,"PURCHASED",false), Triple(false,"UNKNOWN",false))) {
            try { PlayServerEntitlement.parse(owned,state,ack); fail("Accepted invalid server state") }
            catch (_: IllegalArgumentException) {}
        }
    }
    @Test fun offlineOwnershipExpiresAndClockRollbackDoesNotExtendIt() {
        assertTrue(PlayServerEntitlement.fresh(true, 1000, 2000))
        assertFalse(PlayServerEntitlement.fresh(false, 1000, 2000))
        assertFalse(PlayServerEntitlement.fresh(true, 1000, 999))
        assertFalse(PlayServerEntitlement.fresh(true, 0, 2000))
        assertFalse(PlayServerEntitlement.fresh(true, 1000, 86401000))
    }
}
