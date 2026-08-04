package com.warrenbrowse.vpn.lib.repository

import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class MnemonicCacheTest {

    // Drop state between tests since MnemonicCache is a process-global
    // singleton (intentional - models the real handoff between UI and
    // VPN service, which share a process).
    @AfterEach
    fun cleanup() {
        MnemonicCache.put(null)
    }

    private val sample = Mnemonic(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
    )

    @Test
    fun `put then consume returns the staged mnemonic`() {
        MnemonicCache.put(sample)
        val out = MnemonicCache.consume()
        assertEquals(sample.phrase, out?.phrase)
    }

    @Test
    fun `consume is one-shot - second call returns null`() {
        MnemonicCache.put(sample)
        MnemonicCache.consume()
        assertNull(MnemonicCache.consume())
    }

    @Test
    fun `consume without put returns null`() {
        assertNull(MnemonicCache.consume())
    }

    @Test
    fun `put overwrites previous stash without consume`() {
        val a = Mnemonic("legal winner thank year wave sausage worth useful legal winner thank yellow")
        val b = sample
        MnemonicCache.put(a)
        MnemonicCache.put(b)
        assertEquals(b.phrase, MnemonicCache.consume()?.phrase)
    }

    @Test
    fun `isStaged reports presence accurately`() {
        assertFalse(MnemonicCache.isStaged())
        MnemonicCache.put(sample)
        assertTrue(MnemonicCache.isStaged())
        MnemonicCache.consume()
        assertFalse(MnemonicCache.isStaged())
    }

    @Test
    fun `put null clears the cache`() {
        MnemonicCache.put(sample)
        MnemonicCache.put(null)
        assertNull(MnemonicCache.consume())
        assertFalse(MnemonicCache.isStaged())
    }

    @Test
    fun `put overwrites and closes the previously staged mnemonic`() {
        // Invariant: the previous stash must be zeroed when
        // overwritten so its CharArray does not linger on the heap.
        val orphan = Mnemonic(
            "legal winner thank year wave sausage worth useful legal winner thank yellow"
        )
        MnemonicCache.put(orphan)
        MnemonicCache.put(sample)
        // After the overwrite, the orphan must be in the closed state:
        // accessing `.phrase` throws IllegalStateException.
        assertThrows(IllegalStateException::class.java) { orphan.phrase }
    }

    @Test
    fun `put null closes the previously staged mnemonic`() {
        val orphan = Mnemonic(
            "legal winner thank year wave sausage worth useful legal winner thank yellow"
        )
        MnemonicCache.put(orphan)
        MnemonicCache.put(null)
        assertThrows(IllegalStateException::class.java) { orphan.phrase }
    }
}
