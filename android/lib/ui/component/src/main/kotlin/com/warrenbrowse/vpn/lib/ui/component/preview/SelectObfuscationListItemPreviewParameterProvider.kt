package com.warrenbrowse.vpn.lib.ui.component.preview

import androidx.compose.ui.tooling.preview.PreviewParameterProvider
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.ObfuscationMode
import com.warrenbrowse.vpn.lib.model.Port

class SelectObfuscationListItemPreviewParameterProvider :
    PreviewParameterProvider<Triple<ObfuscationMode, Constraint<Port>, Boolean>> {
    override val values: Sequence<Triple<ObfuscationMode, Constraint<Port>, Boolean>> =
        sequenceOf(
            Triple(ObfuscationMode.Shadowsocks, Constraint.Any, false),
            Triple(ObfuscationMode.Shadowsocks, Constraint.Any, true),
            Triple(ObfuscationMode.Shadowsocks, Constraint.Only(Port(PORT)), false),
            Triple(ObfuscationMode.Shadowsocks, Constraint.Only(Port(PORT)), true),
            Triple(ObfuscationMode.Udp2Tcp, Constraint.Any, false),
            Triple(ObfuscationMode.Udp2Tcp, Constraint.Any, true),
            Triple(ObfuscationMode.Udp2Tcp, Constraint.Only(Port(PORT)), false),
            Triple(ObfuscationMode.Udp2Tcp, Constraint.Only(Port(PORT)), true),
        )
}

private const val PORT = 44
