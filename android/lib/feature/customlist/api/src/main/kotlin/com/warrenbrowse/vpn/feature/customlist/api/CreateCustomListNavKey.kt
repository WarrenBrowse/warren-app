package com.warrenbrowse.vpn.feature.customlist.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.NavResult
import com.warrenbrowse.vpn.lib.model.GeoLocationId
import com.warrenbrowse.vpn.lib.model.communication.CustomListActionResultData

@Parcelize data class CreateCustomListNavKey(val locationCode: GeoLocationId? = null) : NavKey2

@Parcelize
data class CreateCustomListNavResult(
    val value: CustomListActionResultData.Success.CreatedWithLocations
) : NavResult
