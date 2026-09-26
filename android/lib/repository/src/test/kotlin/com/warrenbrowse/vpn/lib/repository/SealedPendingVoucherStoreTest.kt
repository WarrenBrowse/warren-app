package com.warrenbrowse.vpn.lib.repository

import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class SealedPendingVoucherStoreTest {

    /**
     * The Keystore sealer's cipher (AES-256-GCM, a fresh nonce ahead of the
     * ciphertext) with a key held in software, since the JVM has no Keystore.
     */
    private class SoftwareSealer : BlobSealer {
        private val key: SecretKey = KeyGenerator.getInstance("AES").apply { init(256) }.generateKey()

        override fun seal(plain: ByteArray): ByteArray {
            val cipher = Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.ENCRYPT_MODE, key) }
            return cipher.iv + cipher.doFinal(plain)
        }

        override fun open(sealed: ByteArray): ByteArray {
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(128, sealed, 0, 12))
            return cipher.doFinal(sealed, 12, sealed.size - 12)
        }
    }

    private class MemorySlot : BlobSlot {
        var blob: ByteArray? = null

        override fun read(): ByteArray? = blob

        override fun write(blob: ByteArray?) {
            this.blob = blob
        }
    }

    private val wallet = "wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB"
    private val voucher = "QWRT-YPLK-JHGF-DSAZ"

    @Test
    fun a_held_voucher_survives_a_restart_and_is_never_stored_in_the_clear() {
        val sealer = SoftwareSealer()
        val slot = MemorySlot()

        SealedPendingVoucherStore(sealer, slot).hold(PendingVoucher(wallet, voucher))

        val stored = String(slot.blob!!, Charsets.ISO_8859_1)
        assertFalse(stored.contains(voucher), "the voucher is on disk in the clear")
        assertFalse(stored.contains(voucher.replace("-", "")))
        // A second store reads it off the slot, as the next process would.
        assertEquals(listOf(PendingVoucher(wallet, voucher)), SealedPendingVoucherStore(sealer, slot).all())
    }

    @Test
    fun holding_the_same_voucher_twice_keeps_one_copy() {
        val store = SealedPendingVoucherStore(SoftwareSealer(), MemorySlot())

        store.hold(PendingVoucher(wallet, voucher))
        store.hold(PendingVoucher(wallet, voucher))

        assertEquals(1, store.all().size)
    }

    @Test
    fun forgetting_the_last_voucher_leaves_nothing_in_the_slot() {
        val slot = MemorySlot()
        val store = SealedPendingVoucherStore(SoftwareSealer(), slot)
        store.hold(PendingVoucher(wallet, voucher))
        store.hold(PendingVoucher(wallet, "ZZZZ-YYYY-XXXX-WWWW"))

        store.forget(voucher)
        assertEquals(listOf(PendingVoucher(wallet, "ZZZZ-YYYY-XXXX-WWWW")), store.all())

        store.forget("ZZZZ-YYYY-XXXX-WWWW")
        assertNull(slot.blob)
        assertTrue(store.all().isEmpty())
    }

    @Test
    fun a_blob_its_key_no_longer_opens_reads_as_nothing_held() {
        val slot = MemorySlot()
        SealedPendingVoucherStore(SoftwareSealer(), slot).hold(PendingVoucher(wallet, voucher))

        assertTrue(SealedPendingVoucherStore(SoftwareSealer(), slot).all().isEmpty())
    }

    @Test
    fun a_held_voucher_never_reaches_a_log_line_through_its_string_form() {
        assertFalse(PendingVoucher(wallet, voucher).toString().contains(voucher))
    }
}
