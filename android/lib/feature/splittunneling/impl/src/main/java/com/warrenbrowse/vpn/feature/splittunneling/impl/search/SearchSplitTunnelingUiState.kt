package com.warrenbrowse.vpn.feature.splittunneling.impl.search

import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.AppData

data class SearchSplitTunnelingUiState(
    val searchTerm: String,
    val excludedApps: List<AppData> = emptyList(),
    val includedApps: List<AppData> = emptyList(),
)
