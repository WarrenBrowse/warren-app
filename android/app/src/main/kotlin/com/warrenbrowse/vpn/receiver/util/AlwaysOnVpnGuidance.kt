package com.warrenbrowse.vpn.receiver.util

/**
 * Whether the OS is holding the network closed for this app across an update.
 *
 * Neither mobile OS lets an app keep the network sealed while it is being
 * replaced: the system tears the tunnel down and no app has the authority to
 * hold it. The desktop protection (arm a lockdown, stage a detached guard) has
 * no equivalent here, so the honest deliverable is detection and guidance
 * toward the one mechanism that DOES cover it, which is a system setting:
 * Always-on VPN with "Block connections without VPN".
 *
 * Deliberately not a synthesized userspace kill switch: that would be a promise
 * the platform does not let us keep.
 */
enum class AlwaysOnVpnGuidance {
    /** The OS blocks connections without this app's VPN. Nothing to say. */
    CONFIGURED,

    /** It is not configured, and we know it. Worth telling the user once. */
    NOT_CONFIGURED,

    /**
     * The setting could not be read. Stay silent: these keys are not public
     * API and a device that hides them must not produce a warning that tells a
     * correctly configured user they are unprotected.
     */
    UNKNOWN,
}

/**
 * Reads the two system values into a verdict.
 *
 * @param alwaysOnVpnPackage the package the OS runs as always-on VPN, or null
 *   when nothing is set or the value could not be read.
 * @param lockdownEnabled whether "Block connections without VPN" is on, or null
 *   when it could not be read.
 * @param ourPackage this app's package name.
 */
fun alwaysOnVpnGuidance(
    alwaysOnVpnPackage: String?,
    lockdownEnabled: Boolean?,
    ourPackage: String,
): AlwaysOnVpnGuidance =
    when {
        // Unreadable on this device: say nothing rather than nag a user who is
        // already protected.
        alwaysOnVpnPackage == null && lockdownEnabled == null -> AlwaysOnVpnGuidance.UNKNOWN
        alwaysOnVpnPackage == ourPackage && lockdownEnabled == true ->
            AlwaysOnVpnGuidance.CONFIGURED
        // Always-on pointed at ANOTHER app is still "not us", and blocking
        // without the lockdown flag still lets traffic out while we are being
        // replaced. Both are worth the same one-time notice.
        else -> AlwaysOnVpnGuidance.NOT_CONFIGURED
    }
