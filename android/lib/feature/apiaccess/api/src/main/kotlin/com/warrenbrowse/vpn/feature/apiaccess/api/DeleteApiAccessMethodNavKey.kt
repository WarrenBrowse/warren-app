package com.warrenbrowse.vpn.feature.apiaccess.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.NavResult
import com.warrenbrowse.vpn.lib.model.ApiAccessMethodId

@Parcelize
data class DeleteApiAccessMethodNavKey(val apiAccessMethodId: ApiAccessMethodId) : NavKey2

@Parcelize object DeleteApiAccessMethodConfirmedNavResult : NavResult
