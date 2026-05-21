package com.warrenbrowse.vpn.feature.location.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.NavResult

@Parcelize object SelectLocationNavKey : NavKey2

@Parcelize data class SelectLocationNavResult(val connect: Boolean) : NavResult
