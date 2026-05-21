package com.warrenbrowse.vpn.feature.customlist.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.NavResult
import com.warrenbrowse.vpn.lib.model.CustomListId
import com.warrenbrowse.vpn.lib.model.CustomListName
import com.warrenbrowse.vpn.lib.model.communication.CustomListActionResultData

@Parcelize
data class EditCustomListNameNavKey(
    val customListId: CustomListId,
    val initialName: CustomListName,
) : NavKey2

@Parcelize
data class EditCustomListNameNavResult(val value: CustomListActionResultData.Success.Renamed) :
    NavResult
