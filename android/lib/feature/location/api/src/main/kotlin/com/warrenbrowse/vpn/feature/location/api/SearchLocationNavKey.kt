package com.warrenbrowse.vpn.feature.location.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.NavResult
import com.warrenbrowse.vpn.lib.model.RelayListType

@Parcelize data class SearchLocationNavKey(val relayListType: RelayListType) : NavKey2

@Parcelize data class SearchLocationNavResult(val relayListType: RelayListType) : NavResult
