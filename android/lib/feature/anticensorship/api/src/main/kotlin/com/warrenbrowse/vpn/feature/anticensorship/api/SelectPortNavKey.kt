package com.warrenbrowse.vpn.feature.anticensorship.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.lib.model.PortType

@Parcelize data class SelectPortNavKey(val portType: PortType) : NavKey2
