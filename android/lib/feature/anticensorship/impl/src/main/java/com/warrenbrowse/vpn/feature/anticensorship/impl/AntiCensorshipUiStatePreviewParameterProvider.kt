package com.warrenbrowse.vpn.feature.anticensorship.impl

import androidx.compose.ui.tooling.preview.PreviewParameterProvider
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.toLc
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.ObfuscationMode

class AntiCensorshipUiStatePreviewParameterProvider :
    PreviewParameterProvider<Lc<Boolean, AntiCensorshipSettingsUiState>> {
    override val values =
        sequenceOf(
            AntiCensorshipSettingsUiState.from(
                    isModal = false,
                    selectedWireguardPort = Constraint.Any,
                    obfuscationMode = ObfuscationMode.Udp2Tcp,
                    selectedUdp2TcpObfuscationPort = Constraint.Any,
                    selectedShadowsocksObfuscationPort = Constraint.Any,
                )
                .toLc(),
            Lc.Loading(true),
        )
}
