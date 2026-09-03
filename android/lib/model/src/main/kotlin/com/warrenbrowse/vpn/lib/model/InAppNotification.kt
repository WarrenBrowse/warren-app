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

    /**
     * The operator has published a launch announcement, and it may carry the
     * voucher code drawn for this account. First of all, above the operator
     * notice itself (desktop `NotificationArea`): a warning or an error notice
     * keeps the slot for as long as it stands, and the card carries a code that
     * stops being worth anything once the campaign closes. The card steps aside
     * on its own the moment the reader puts it away, and the notice is then the
     * banner that shows.
     */
    data class LaunchAnnouncement(val announcement: WarrenAnnouncement) : InAppNotification() {
        override val statusLevel =
            when (announcement.level) {
                WarrenNoticeLevel.ERROR -> StatusLevel.Error
                WarrenNoticeLevel.WARNING -> StatusLevel.Warning
                WarrenNoticeLevel.INFO -> StatusLevel.Info
            }

        override val priority: Long = 1010
    }

    /**
     * The operator has published a broadcast notice. Ranked above every other
     * banner but the launch announcement, the stand-down included: when the
     * operator has something to say to every user, that message is the one
     * thing they must read, and the states it hides (connecting, offline,
     * blocked) are still legible in the connect card's own status.
     *
     * It clears from the signal that raised it: Rust hands over an empty list
     * as soon as the notice is erased or its signed envelope lapses (desktop
     * `WarrenNoticeNotificationProvider`). An informational one can also be put
     * away by the reader, so a message the operator leaves up for a week does
     * not hide the update prompt and the expiry warning for that whole week.
     */
    data class OperatorNotice(val notice: WarrenNotice) : InAppNotification() {
        override val statusLevel =
            when (notice.level) {
                WarrenNoticeLevel.ERROR -> StatusLevel.Error
                WarrenNoticeLevel.WARNING -> StatusLevel.Warning
                WarrenNoticeLevel.INFO -> StatusLevel.Info
            }

        override val priority: Long = 1009
    }

    // A higher-priority product environment (prod over staging over beta) is
    // installed on this device, so this build took its tunnel down and will not
    // bring it back until the user says so. Ranked above every tunnel banner
    // because it explains why this build is not connecting at all: a banner
    // about the connection would describe a state it is not trying to reach.
    // A deliberate stand-down rather than a failure, so it is a warning, not an
    // error.
    data object EnvStandDown : InAppNotification() {
        override val statusLevel = StatusLevel.Warning
        override val priority: Long = 1008
    }

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

    // An automatic retry landed on another exit than the one that dropped
    // (desktop WarrenFailoverNotificationProvider). Stays up until dismissed,
    // so a switch that happened while the screen was away is still read.
    data object ExitSwitched : InAppNotification() {
        override val statusLevel = StatusLevel.Warning
        override val priority: Long = 1001
    }

    /**
     * Subscription running out. [daysLeft] is 0 once the expiry is past, which
     * is the only case coded as an error: an expiry that is merely approaching
     * is a warning, and red is reserved for a tunnel that is failing now.
     */
    data class CloseToExpiry(val daysLeft: Long) : InAppNotification() {
        override val statusLevel =
            if (daysLeft <= 0L) StatusLevel.Error else StatusLevel.Warning
        override val priority: Long = 1000
    }

    // Right after an install the changelog outranks a fresh update prompt, so
    // the user reads what just landed instead of being asked to update again.
    data object NewVersionChangelog : InAppNotification() {
        override val statusLevel = StatusLevel.Info
        override val priority: Long = 999
    }

    data class UpdateAvailable(val version: String) : InAppNotification() {
        override val statusLevel = StatusLevel.Warning
        override val priority: Long = 998
    }
}
