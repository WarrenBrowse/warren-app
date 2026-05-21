package com.warrenbrowse.vpn.feature.home.impl.devicerevoked

import androidx.compose.ui.tooling.preview.PreviewParameterProvider

class DeviceRevokedUiStatePreviewParameterProvider :
    PreviewParameterProvider<DeviceRevokedUiState> {
    override val values =
        sequenceOf(
            DeviceRevokedUiState.SECURED,
            DeviceRevokedUiState.UNSECURED,
            DeviceRevokedUiState.UNKNOWN,
        )
}
