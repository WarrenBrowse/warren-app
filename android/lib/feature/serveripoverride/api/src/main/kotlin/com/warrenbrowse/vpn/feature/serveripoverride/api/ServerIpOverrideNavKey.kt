package com.warrenbrowse.vpn.feature.serveripoverride.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2

@Parcelize data class ServerIpOverrideNavKey(val isModal: Boolean = false) : NavKey2
