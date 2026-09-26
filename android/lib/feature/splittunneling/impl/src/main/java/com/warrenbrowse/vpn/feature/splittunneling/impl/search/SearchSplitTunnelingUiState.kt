package com.warrenbrowse.vpn.feature.splittunneling.impl.search

import com.warrenbrowse.vpn.feature.splittunneling.impl.SplitTunnelingTab
import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.AppData

data class SearchSplitTunnelingUiState(
    val searchTerm: String,
    val tab: SplitTunnelingTab = SplitTunnelingTab.Bypass,
    val selectedApps: List<AppData> = emptyList(),
    val otherApps: List<AppData> = emptyList(),
)
