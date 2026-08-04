package com.warrenbrowse.vpn.feature.splittunneling.impl

import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.AppData

data class Loading(val isModal: Boolean = false)

data class SplitTunnelingUiState(
    val enabled: Boolean = false,
    val excludedApps: List<AppData> = emptyList(),
    val includedApps: List<AppData> = emptyList(),
    val showSystemApps: Boolean = false,
    val isModal: Boolean = false,
)
