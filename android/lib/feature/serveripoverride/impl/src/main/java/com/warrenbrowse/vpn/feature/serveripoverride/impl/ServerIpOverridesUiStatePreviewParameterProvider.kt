package com.warrenbrowse.vpn.feature.serveripoverride.impl

import androidx.compose.ui.tooling.preview.PreviewParameterProvider
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.toLc

class ServerIpOverridesUiStatePreviewParameterProvider :
    PreviewParameterProvider<Lc<Boolean, ServerIpOverridesUiState>> {
    override val values =
        sequenceOf(
            ServerIpOverridesUiState(overridesActive = true).toLc(),
            ServerIpOverridesUiState(overridesActive = false).toLc(),
            Lc.Loading(true),
        )
}
