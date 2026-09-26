package com.warrenbrowse.vpn.lib.repository

import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertThrows
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

    private class MemoryShelf : BlobShelf {
        val blobs = linkedMapOf<String, ByteArray>()
        var refuseWrites = false

        override fun all(): Map<String, ByteArray> = LinkedHashMap(blobs)

        override fun put(name: String, blob: ByteArray) {
            if (refuseWrites) throw java.io.IOException("disk full")
            blobs[name] = blob
        }

        override fun remove(name: String) {
            blobs.remove(name)
        }
    }

    private val wallet = "wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB"
    private val voucher = "QWRT-YPLK-JHGF-DSAZ"

    @Test
    fun a_held_voucher_survives_a_restart_and_is_never_stored_in_the_clear() {
        val sealer = SoftwareSealer()
        val shelf = MemoryShelf()

        SealedPendingVoucherStore(sealer, shelf).hold(PendingVoucher(wallet, voucher))

        val stored = shelf.blobs.entries.joinToString { it.key + String(it.value, Charsets.ISO_8859_1) }
        assertFalse(stored.contains(voucher), "the voucher is on disk in the clear")
        assertFalse(stored.contains(voucher.replace("-", "")))
        // A second store reads it off the shelf, as the next process would.
        assertEquals(listOf(PendingVoucher(wallet, voucher)), SealedPendingVoucherStore(sealer, shelf).all())
    }

    @Test
    fun holding_the_same_voucher_twice_keeps_one_copy() {
        val store = SealedPendingVoucherStore(SoftwareSealer(), MemoryShelf())

        store.hold(PendingVoucher(wallet, voucher))
        store.hold(PendingVoucher(wallet, voucher))

        assertEquals(1, store.all().size)
    }

    @Test
    fun forgetting_the_last_voucher_leaves_nothing_on_the_shelf() {
        val shelf = MemoryShelf()
        val store = SealedPendingVoucherStore(SoftwareSealer(), shelf)
        store.hold(PendingVoucher(wallet, voucher))
        store.hold(PendingVoucher(wallet, "ZZZZ-YYYY-XXXX-WWWW"))

        store.forget(voucher)
        assertEquals(listOf(PendingVoucher(wallet, "ZZZZ-YYYY-XXXX-WWWW")), store.all())

        store.forget("ZZZZ-YYYY-XXXX-WWWW")
        assertTrue(shelf.blobs.isEmpty())
        assertTrue(store.all().isEmpty())
    }

    @Test
    fun a_voucher_its_key_cannot_open_now_is_neither_listed_nor_overwritten() {
        // A Keystore that fails once (busy, or a key dropped for good) must not
        // cost the vouchers it sealed: the next purchase adds its own entry.
        val shelf = MemoryShelf()
        SealedPendingVoucherStore(SoftwareSealer(), shelf).hold(PendingVoucher(wallet, voucher))
        val sealedBefore = shelf.blobs.toMap()
        val other = SealedPendingVoucherStore(SoftwareSealer(), shelf)

        assertTrue(other.all().isEmpty())
        other.hold(PendingVoucher(wallet, "ZZZZ-YYYY-XXXX-WWWW"))
        other.forget(voucher)

        sealedBefore.forEach { (name, blob) -> assertTrue(shelf.blobs[name].contentEquals(blob)) }
        assertEquals(2, shelf.blobs.size)
    }

    @Test
    fun a_voucher_the_disk_refused_is_reported_before_anyone_redeems_it() {
        val shelf = MemoryShelf().apply { refuseWrites = true }
        val store = SealedPendingVoucherStore(SoftwareSealer(), shelf)

        assertThrows(java.io.IOException::class.java) { store.hold(PendingVoucher(wallet, voucher)) }
    }

    @Test
    fun a_held_voucher_never_reaches_a_log_line_through_its_string_form() {
        assertFalse(PendingVoucher(wallet, voucher).toString().contains(voucher))
    }
}
