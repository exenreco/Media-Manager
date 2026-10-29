package dev.dioxus.main

import org.junit.Assert.*
import org.junit.Test
import java.security.KeyPairGenerator
import java.security.Signature

class PlayPurchaseSignatureTest {
    @Test fun verifiesSignedReceiptAndRejectsTampering() {
        val keys = KeyPairGenerator.getInstance("RSA").apply { initialize(2048) }.generateKeyPair()
        val receipt = """{"productId":"remove_ads","purchaseState":0}""".toByteArray()
        val signature = Signature.getInstance("SHA1withRSA").run {
            initSign(keys.private); update(receipt); sign()
        }
        assertTrue(PlayPurchaseSignature.verify(keys.public, receipt, signature))
        assertFalse(PlayPurchaseSignature.verify(keys.public, "changed".toByteArray(), signature))
        val wrongKey = KeyPairGenerator.getInstance("RSA").apply { initialize(2048) }.generateKeyPair()
        assertFalse(PlayPurchaseSignature.verify(wrongKey.public, receipt, signature))
        assertFalse(PlayPurchaseSignature.verify(keys.public, receipt, byteArrayOf(1, 2, 3)))
    }
}
