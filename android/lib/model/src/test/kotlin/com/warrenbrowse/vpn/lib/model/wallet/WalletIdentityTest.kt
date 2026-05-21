package com.warrenbrowse.vpn.lib.model.wallet

import kotlin.test.assertEquals
import kotlin.test.assertNotEquals
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertDoesNotThrow
import org.junit.jupiter.api.assertThrows

class WalletPubkeyHexTest {
    @Test
    fun `accepts 64-char hex value`() {
        assertDoesNotThrow { WalletPubkeyHex("a".repeat(64)) }
    }

    @Test
    fun `rejects shorter value`() {
        assertThrows<IllegalArgumentException> { WalletPubkeyHex("a".repeat(63)) }
    }

    @Test
    fun `rejects longer value`() {
        assertThrows<IllegalArgumentException> { WalletPubkeyHex("a".repeat(65)) }
    }

    @Test
    fun `rejects empty string`() {
        assertThrows<IllegalArgumentException> { WalletPubkeyHex("") }
    }
}

class MnemonicTest {
    private val twelveWords =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
    private val twentyFourWords =
        "abandon abandon abandon abandon abandon abandon abandon abandon " +
            "abandon abandon abandon abandon abandon abandon abandon abandon " +
            "abandon abandon abandon abandon abandon abandon abandon art"

    @Test
    fun `accepts 12-word phrase`() {
        assertDoesNotThrow { Mnemonic(twelveWords) }
    }

    @Test
    fun `accepts 24-word phrase`() {
        assertDoesNotThrow { Mnemonic(twentyFourWords) }
    }

    @Test
    fun `rejects 11-word phrase`() {
        val phrase = twelveWords.split(' ').take(11).joinToString(" ")
        assertThrows<IllegalArgumentException> { Mnemonic(phrase) }
    }

    @Test
    fun `rejects 13-word phrase`() {
        val phrase = "$twelveWords abandon"
        assertThrows<IllegalArgumentException> { Mnemonic(phrase) }
    }

    @Test
    fun `rejects empty phrase`() {
        assertThrows<IllegalArgumentException> { Mnemonic("") }
    }

    @Test
    fun `toString does not leak the mnemonic`() {
        val mnemonic = Mnemonic(twelveWords)
        // The class must NEVER include the phrase in its string form -
        // otherwise interpolation in a log statement would leak it.
        assertEquals("Mnemonic(<redacted>)", mnemonic.toString())
        assertNotEquals(twelveWords, mnemonic.toString())
        assertEquals(false, mnemonic.toString().contains("abandon"))
    }
}
