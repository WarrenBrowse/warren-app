package com.warrenbrowse.vpn.feature.serveripoverride.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.NavResult

@Parcelize data object ResetServerIpOverrideConfirmationNavKey : NavKey2

@Parcelize
data class ResetServerIpOverrideConfirmationNavResult(val clearSuccessful: Boolean) : NavResult
