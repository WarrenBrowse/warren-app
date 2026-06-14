package com.warrenbrowse.vpn.feature.settings.impl

import androidx.compose.ui.tooling.preview.PreviewParameterProvider
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.toLc

class SettingsUiStatePreviewParameterProvider :
    PreviewParameterProvider<Lc<Unit, SettingsUiState>> {
    override val values =
        sequenceOf(
            Lc.Loading(Unit),
            SettingsUiState(
                    appVersion = "2222.22",
                    isLoggedIn = true,
                    isSupportedVersion = true,
                    isDaitaEnabled = true,
                    isPlayBuild = true,
                )
                .toLc(),
            SettingsUiState(
                    appVersion = "9000.1",
                    isLoggedIn = false,
                    isSupportedVersion = false,
                    isDaitaEnabled = false,
                    isPlayBuild = false,
                )
                .toLc(),
        )
}
