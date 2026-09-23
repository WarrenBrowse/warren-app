package com.warrenbrowse.vpn.lib.repository

import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class PurchaseClaimTest {
    @Test
    fun `the hash is the SHA-256 of the raw secret bytes, as warren-api computes it`() {
        val claim = PurchaseClaim("0".repeat(32), "11".repeat(32))
        assertEquals(
            "02d449a31fbb267c8f352e9968a79e3e5fc95c1bbeaa502fd6454ebde5a4bedc",
            claim.pullSecretHash,
        )
    }

    @Test
    fun `the checkout URL carries the wpid and the hash, never the secret`() {
        val claim = PurchaseClaim.mint()
        val url = claim.checkoutUrl("https://checkout.warrenbrowse.com/")
        assertEquals(
            "https://checkout.warrenbrowse.com/?pid=${claim.wpid}&ph=${claim.pullSecretHash}",
            url,
        )
        assertFalse(url.contains(claim.code.substring(32)))
        assertFalse(claim.toString().contains(claim.code.substring(32)))
    }

    @Test
    fun `a minted claim is a fresh 32-hex wpid and a 96-hex code`() {
        val a = PurchaseClaim.mint()
        val b = PurchaseClaim.mint()
        assertTrue(Regex("^[0-9a-f]{32}$").matches(a.wpid))
        assertTrue(Regex("^[0-9a-f]{96}$").matches(a.code))
        assertTrue(a.code.startsWith(a.wpid))
        assertNotEquals(a.code, b.code)
    }
}
