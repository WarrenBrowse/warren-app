package com.warrenbrowse.vpn.lib.repository

import java.security.MessageDigest
import java.security.SecureRandom

/**
 * What the app keeps to collect a purchase it opened in the browser (warren-core doc 35
 * section 14). The [wpid] names the purchase and rides in the checkout URL, so it proves
 * nothing on its own. The pull secret is what warren-api asks for before it hands out the
 * voucher: only its SHA-256 goes to the checkout (the `ph` parameter), and the secret itself
 * leaves the app only inside the pull, through [code].
 */
class PurchaseClaim internal constructor(val wpid: String, private val pullSecret: String) {
    /** SHA-256 of the raw secret bytes, the digest warren-api stores. */
    val pullSecretHash: String
        get() = MessageDigest.getInstance("SHA-256").digest(pullSecret.hexToBytes()).toHex()

    /**
     * The wpid followed by the pull secret (96 hex chars): the form `WarrenJni.redeemVoucher`
     * recognizes as a purchase to collect rather than a voucher to redeem.
     */
    val code: String
        get() = wpid + pullSecret

    /** The checkout page for this purchase; the account chip fragment is the caller's. */
    fun checkoutUrl(base: String): String = "$base?pid=$wpid&ph=$pullSecretHash"

    // The secret must never reach a log line through string interpolation.
    override fun toString(): String = "PurchaseClaim([redacted])"

    companion object {
        /** A fresh 128-bit wpid and 256-bit pull secret. */
        fun mint(): PurchaseClaim {
            val random = SecureRandom()
            return PurchaseClaim(random.hex(16), random.hex(32))
        }

        private fun SecureRandom.hex(bytes: Int): String =
            ByteArray(bytes).also { nextBytes(it) }.toHex()
    }
}

private fun ByteArray.toHex(): String = joinToString("") { "%02x".format(it) }

private fun String.hexToBytes(): ByteArray =
    ByteArray(length / 2) { i -> substring(2 * i, 2 * i + 2).toInt(16).toByte() }
