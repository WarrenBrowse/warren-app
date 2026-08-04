package com.warrenbrowse.vpn.lib.model

data class VersionInfo(
    val currentVersion: String,
    val isSupported: Boolean,
    // Newest stable version newer than [currentVersion], or null when none is
    // known. Drives the sideload-only "update available" notification.
    val availableUpgrade: String? = null,
)
