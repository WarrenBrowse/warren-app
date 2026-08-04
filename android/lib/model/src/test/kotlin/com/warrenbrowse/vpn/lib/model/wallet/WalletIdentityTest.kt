package com.warrenbrowse.vpn.lib.model.wallet

import kotlin.test.assertEquals
import kotlin.test.assertNotEquals
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertDoesNotThrow
import org.junit.jupiter.api.assertThrows

class WalletAddressTest {
    // Real Warren SS58 vector (49 chars, prefix 13295).
    private val validAddress = "wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB"

    @Test
    fun `accepts a real Warren SS58 address`() {
        assertDoesNotThrow { WalletAddress(validAddress) }
    }

    @Test
    fun `accepts boundary lengths 47 and 49`() {
        // Pad / trim the real vector to the inclusive length bounds while
        // keeping the wb prefix and a base58 charset.
        val body = validAddress.drop(2)
        assertDoesNotThrow { WalletAddress("wb" + body.take(45)) } // 47 chars
        assertDoesNotThrow { WalletAddress("wb" + body.take(45) + "ab") } // 49 chars
    }

    @Test
    fun `rejects address not starting with wb`() {
        assertThrows<IllegalArgumentException> {
            WalletAddress("xy7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB")
        }
    }

    @Test
    fun `rejects too short`() {
        assertThrows<IllegalArgumentException> { WalletAddress("wb" + "a".repeat(44)) } // 46
    }

    @Test
    fun `rejects too long`() {
        assertThrows<IllegalArgumentException> { WalletAddress("wb" + "a".repeat(48)) } // 50
    }

    @Test
    fun `rejects non-base58 characters`() {
        // `0`, `O`, `I`, `l` are excluded from the base58 alphabet.
        assertThrows<IllegalArgumentException> {
            WalletAddress("wb0OIl" + "a".repeat(42)) // 48 chars but illegal glyphs
        }
    }

    @Test
    fun `rejects empty string`() {
        assertThrows<IllegalArgumentException> { WalletAddress("") }
    }

    @Test
    fun `short form is first 6 then ellipsis then last 6`() {
        assertEquals("wb7kgy…hP9DnB", validAddress.shortWarrenAddress())
    }

    @Test
    fun `short form returns short strings unchanged`() {
        assertEquals("wb7kgy", "wb7kgy".shortWarrenAddress())
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

    @Test
    fun `close throws on subsequent phrase read`() {
        val mnemonic = Mnemonic(twelveWords)
        mnemonic.close()
        assertThrows<IllegalStateException> { mnemonic.phrase }
    }

    @Test
    fun `close is idempotent`() {
        val mnemonic = Mnemonic(twelveWords)
        mnemonic.close()
        // A second close MUST NOT throw - matches the AutoCloseable
        // contract that callers can safely close-in-finally even when
        // the body already closed the resource.
        mnemonic.close()
    }

    @Test
    fun `phrase round-trips through CharArray backing store`() {
        val mnemonic = Mnemonic(twelveWords)
        assertEquals(twelveWords, mnemonic.phrase)
    }

    @Test
    fun `useAsString delivers the phrase to the block`() {
        val mnemonic = Mnemonic(twelveWords)
        val observed = mnemonic.useAsString { it }
        assertEquals(twelveWords, observed)
    }
}
