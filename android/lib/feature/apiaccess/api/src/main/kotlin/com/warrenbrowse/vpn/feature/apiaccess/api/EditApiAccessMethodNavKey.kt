package com.warrenbrowse.vpn.feature.apiaccess.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.NavResult
import com.warrenbrowse.vpn.lib.model.ApiAccessMethodId

@Parcelize
data class EditApiAccessMethodNavKey(val accessMethodId: ApiAccessMethodId? = null) : NavKey2

@Parcelize data class EditApiAccessMethodNavResult(val success: Boolean) : NavResult
