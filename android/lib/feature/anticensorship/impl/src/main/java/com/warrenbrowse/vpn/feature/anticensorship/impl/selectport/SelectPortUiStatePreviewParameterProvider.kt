package com.warrenbrowse.vpn.feature.anticensorship.impl.selectport

import androidx.compose.ui.tooling.preview.PreviewParameterProvider
import com.warrenbrowse.vpn.feature.anticensorship.impl.UDP2TCP_PRESET_PORTS
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.toLc
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.Port
import com.warrenbrowse.vpn.lib.model.PortType

class SelectPortUiStatePreviewParameterProvider :
    PreviewParameterProvider<Lc<Unit, SelectPortUiState>> {
    override val values: Sequence<Lc<Unit, SelectPortUiState>> =
        sequenceOf(
            SelectPortUiState(
                    portType = PortType.Udp2Tcp,
                    presetPorts = UDP2TCP_PRESET_PORTS,
                    customPortEnabled = false,
                    title = "Select port",
                )
                .toLc(),
            SelectPortUiState(
                    portType = PortType.Lwo,
                    port = Constraint.Only(Port(1)),
                    presetPorts = emptyList(),
                    customPortEnabled = true,
                    title = "Select port",
                )
                .toLc(),
        )
}
