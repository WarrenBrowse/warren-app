package com.warrenbrowse.vpn.feature.anticensorship.impl.selectport

import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.Port
import com.warrenbrowse.vpn.lib.model.PortRange
import com.warrenbrowse.vpn.lib.model.PortType

data class SelectPortUiState(
    val portType: PortType,
    val port: Constraint<Port> = Constraint.Any,
    val customPort: Port? = null,
    val customPortEnabled: Boolean,
    val title: String,
    val allowedPortRanges: List<PortRange> = emptyList(),
    val recommendedPortRanges: List<PortRange> = emptyList(),
    val presetPorts: List<Port> = emptyList(),
    val infoDestination: NavKey2? = null,
) {
    val isCustom = port is Constraint.Only && port.value !in presetPorts
}

data class PortTypeUiState(
    val presetPorts: List<Port>,
    val allowedPortRanges: List<PortRange>,
    val recommendedPortRanges: List<PortRange>,
    val customPortEnabled: Boolean,
    val title: String,
    val infoDestination: NavKey2? = null,
)
