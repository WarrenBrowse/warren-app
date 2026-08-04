package com.warrenbrowse.vpn.lib.model

enum class StatusLevel {
    Error,
    Warning,
    Info,
    None,
}

/**
 * The single-slot in-app banner. Exactly one notification is shown at a time,
 * so every surface that needs the banner competes here rather than stacking its
 * own strip above it. The priority ladder mirrors the desktop provider order in
 * `NotificationArea.tsx`, where the first provider that may display wins.
 */
sealed class InAppNotification {
    abstract val statusLevel: StatusLevel
    abstract val priority: Long

    // Shown in every tunnel state while the device has no usable network:
    // the tunnel machinery can hold Connected through its redial window, so
    // the offline verdict must outrank every tunnel-state banner (desktop
    // parity: the "NO INTERNET CONNECTION" banner outranks BLOCKED).
    data object HostOffline : InAppNotification() {
        override val statusLevel = StatusLevel.Error
        override val priority: Long = 1007
    }

    // Same rank family as HostOffline, distinct cause: the host network is
    // fine but the exit stopped forwarding. Both degrade the card to
    // "Connection interrupted", and only the banner tells the two apart.
    data object ExitEgressDead : InAppNotification() {
        override val statusLevel = StatusLevel.Error
        override val priority: Long = 1006
    }

    data class TunnelStateError(val error: ErrorState) : InAppNotification() {
        override val statusLevel =
            if (error.cause is ErrorStateCause.IsOffline) {
                StatusLevel.Warning
            } else {
                StatusLevel.Error
            }
        override val priority: Long = 1005
    }

    data object Android16UpgradeWarning : InAppNotification() {
        override val statusLevel = StatusLevel.Warning
        override val priority: Long = 1005
    }

    // Ranked above the plain blocked banner so a stalled attempt upgrades to
    // the help hint instead of spinning on "BLOCKING INTERNET" forever.
    data object ConnectingStuck : InAppNotification() {
        override val statusLevel = StatusLevel.Warning
        override val priority: Long = 1004
    }

    data object TunnelStateBlocked : InAppNotification() {
        override val statusLevel = StatusLevel.None
        override val priority: Long = 1003
    }

    data class UnsupportedVersion(val versionInfo: VersionInfo) : InAppNotification() {
        override val statusLevel = StatusLevel.Error
        override val priority: Long = 1002
    }

    /**
     * Subscription running out. [daysLeft] is 0 once the expiry is past, which
     * is the only case coded as an error: an expiry that is merely approaching
     * is a warning, and red is reserved for a tunnel that is failing now.
     */
    data class CloseToExpiry(val daysLeft: Long) : InAppNotification() {
        override val statusLevel =
            if (daysLeft <= 0L) StatusLevel.Error else StatusLevel.Warning
        override val priority: Long = 1001
    }

    // Right after an install the changelog outranks a fresh update prompt, so
    // the user reads what just landed instead of being asked to update again.
    data object NewVersionChangelog : InAppNotification() {
        override val statusLevel = StatusLevel.Info
        override val priority: Long = 1000
    }

    data class UpdateAvailable(val version: String) : InAppNotification() {
        override val statusLevel = StatusLevel.Warning
        override val priority: Long = 999
    }
}
