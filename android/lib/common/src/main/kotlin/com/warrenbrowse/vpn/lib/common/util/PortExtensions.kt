package com.warrenbrowse.vpn.lib.common.util

import com.warrenbrowse.vpn.lib.model.Port
import com.warrenbrowse.vpn.lib.model.PortRange

fun Port.inAnyOf(portRanges: List<PortRange>): Boolean = portRanges.any { portRange ->
    this in portRange
}

fun List<PortRange>.asString() = joinToString(", ", transform = PortRange::toFormattedString)
