package com.warrenbrowse.vpn.lib.common.constant

import java.security.MessageDigest
import java.security.SecureRandom

/**
 * A secret this process puts on the intents it sends to its own exported
 * activity, so that activity can tell them from the same action sent by any
 * other app on the device. The activity has to be exported to be launchable,
 * which means every installed app may send it an explicit intent: an action
 * that connects the VPN must not be one of those they can trigger.
 *
 * The secret lives as long as the process. An intent built by an earlier run
 * carries an older one and is ignored, which only happens to a notification
 * that outlived its process.
 */
object InternalIntentToken {
    /** The extra that carries the secret. */
    const val EXTRA = "$WARREN_PACKAGE_NAME.internal_intent_token"

    private const val SECRET_BYTES = 32

    /** The secret of this process, to put on an intent under [EXTRA]. */
    val value: String =
        ByteArray(SECRET_BYTES).also { SecureRandom().nextBytes(it) }.joinToString("") {
            "%02x".format(it)
        }

    /** Whether [presented] is this process' secret, compared in constant time. */
    fun isGenuine(presented: String?): Boolean =
        presented != null &&
            MessageDigest.isEqual(presented.toByteArray(), value.toByteArray())
}
