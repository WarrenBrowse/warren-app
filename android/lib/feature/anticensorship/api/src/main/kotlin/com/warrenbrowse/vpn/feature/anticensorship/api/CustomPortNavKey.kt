package com.warrenbrowse.vpn.feature.anticensorship.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.NavResult
import com.warrenbrowse.vpn.lib.model.Port
import com.warrenbrowse.vpn.lib.model.PortRange
import com.warrenbrowse.vpn.lib.model.PortType

@Parcelize
data class CustomPortNavKey(
    val portType: PortType,
    val allowedPortRanges: List<PortRange>,
    val recommendedPortRanges: List<PortRange>,
    val customPort: Port?,
) : NavKey2

@Parcelize data class CustomPortNavResult(val port: Port?) : NavResult
