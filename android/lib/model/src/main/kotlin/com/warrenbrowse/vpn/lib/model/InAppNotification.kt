package com.warrenbrowse.vpn.lib.model

enum class StatusLevel {
    Error,
    Warning,
    Info,
    None,
}

sealed class InAppNotification {
    abstract val statusLevel: StatusLevel
    abstract val priority: Long

    // Shown in every tunnel state while the device has no usable network:
    // the tunnel machinery can hold Connected through its redial window, so
    // the offline verdict must outrank every tunnel-state banner (desktop
    // parity: the "NO INTERNET CONNECTION" banner outranks BLOCKED).
    data object HostOffline : InAppNotification() {
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

    data object TunnelStateBlocked : InAppNotification() {
        override val statusLevel = StatusLevel.None
        override val priority: Long = 1004
    }

    data class UnsupportedVersion(val versionInfo: VersionInfo) : InAppNotification() {
        override val statusLevel = StatusLevel.Error
        override val priority: Long = 1002
    }

    data object NewVersionChangelog : InAppNotification() {
        override val statusLevel = StatusLevel.Info
        override val priority: Long = 1000
    }

    data class UpdateAvailable(val version: String) : InAppNotification() {
        override val statusLevel = StatusLevel.Info
        override val priority: Long = 1001
    }
}
