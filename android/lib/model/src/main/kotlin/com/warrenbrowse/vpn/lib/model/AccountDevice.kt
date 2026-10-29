package com.warrenbrowse.vpn.lib.model

import java.time.LocalDate

/**
 * One device of the logged-in account, as the registry lists it (warren-core doc 115). It names an
 * install of the account and the days it was used: it belongs on the account's own screens and
 * never in a log.
 */
data class AccountDevice(
    val id: String,
    /** As the registry keeps it; shown through [displayedName]. */
    val name: String,
    /** The registry's spelling (`android`, `ios`, `macos`, `windows`, `linux`, `cli`, `browser`). */
    val platform: String,
    val appVersion: String,
    /** The last UTC day it was seen, `null` when the registry wrote one this client cannot read. */
    val lastSeenOn: LocalDate?,
    val thisDevice: Boolean,
) {
    /** The name without bidirectional controls, which another device chose. */
    val displayedName: String
        get() = displayedDeviceName(name)
}

data class AccountDeviceList(
    val devices: List<AccountDevice>,
    /**
     * The last day a device of this account took the place of a Warren client too old to
     * register; `null` when that never happened.
     */
    val olderClientSeenOn: LocalDate?,
)

/** Why a device registry call did nothing. */
enum class DeviceRegistryError {
    /** No wallet is on the device. */
    NoWallet,
    /** A malformed device id. */
    InvalidDevice,
    /** A name the registry refuses. */
    InvalidName,
    /** The account has no such active device. */
    NotFound,
    /** This device was revoked from another device of the account. */
    Revoked,
    /** Nothing happened; a retry can work. */
    Unavailable;

    companion object {
        /** The error code of a native envelope; an unknown one reads as [Unavailable]. */
        fun fromCode(code: String?): DeviceRegistryError =
            when (code) {
                "no_wallet" -> NoWallet
                "invalid_device" -> InvalidDevice
                "invalid_name" -> InvalidName
                "not_found" -> NotFound
                "revoked" -> Revoked
                else -> Unavailable
            }
    }
}

/** The longest name the registry keeps, in characters. */
const val DEVICE_NAME_MAX_CHARS = 40

// Explicit embeddings, overrides and isolates (U+202A to U+202E, U+2066 to U+2069): a name another
// device chose could otherwise reorder the text around it.
private val BIDI_CONTROLS = Regex("[\u202A-\u202E\u2066-\u2069]")

fun displayedDeviceName(name: String): String = name.replace(BIDI_CONTROLS, "")

/**
 * Whether the registry keeps this name: 1 to 40 characters (code points, as the registry counts
 * them) once trimmed, and no control character.
 */
fun isValidDeviceName(name: String): Boolean {
    val trimmed = name.trim()
    val length = trimmed.codePointCount(0, trimmed.length)
    return length in 1..DEVICE_NAME_MAX_CHARS &&
        trimmed.codePoints().noneMatch { Character.getType(it) == Character.CONTROL.toInt() }
}

/** This device first, then the others by last activity, the most recent first, then by name. */
fun List<AccountDevice>.inDisplayOrder(): List<AccountDevice> =
    sortedWith(
        compareByDescending<AccountDevice> { it.thisDevice }
            .thenByDescending { it.lastSeenOn }
            .thenBy { it.displayedName }
    )
