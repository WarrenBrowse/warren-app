package com.warrenbrowse.vpn.feature.apiaccess.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.lib.model.ApiAccessMethodId

@Parcelize data class ApiAccessMethodDetailsNavKey(val accessMethodId: ApiAccessMethodId) : NavKey2
