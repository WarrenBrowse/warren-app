package com.warrenbrowse.vpn.feature.apiaccess.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.NavResult
import com.warrenbrowse.vpn.lib.model.ApiAccessMethod
import com.warrenbrowse.vpn.lib.model.ApiAccessMethodId
import com.warrenbrowse.vpn.lib.model.ApiAccessMethodName

@Parcelize
data class SaveApiAccessMethodNavKey(
    val id: ApiAccessMethodId?,
    val name: ApiAccessMethodName,
    val customProxy: ApiAccessMethod.CustomProxy,
) : NavKey2

@Parcelize data class SaveApiAccessMethodNavResult(val success: Boolean) : NavResult
