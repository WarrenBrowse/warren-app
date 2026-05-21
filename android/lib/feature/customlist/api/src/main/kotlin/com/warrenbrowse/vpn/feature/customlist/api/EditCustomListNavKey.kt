package com.warrenbrowse.vpn.feature.customlist.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.NavResult
import com.warrenbrowse.vpn.lib.model.CustomListId
import com.warrenbrowse.vpn.lib.model.communication.CustomListActionResultData

@Parcelize data class EditCustomListNavKey(val customListId: CustomListId) : NavKey2

@Parcelize data class EditCustomListNavResult(val value: CustomListActionResultData) : NavResult
