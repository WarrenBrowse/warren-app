package com.warrenbrowse.vpn.feature.appinfo.impl

import androidx.compose.ui.tooling.preview.PreviewParameterProvider
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.toLc
import com.warrenbrowse.vpn.lib.model.VersionInfo

class AppInfoUiStatePreviewParameterProvider : PreviewParameterProvider<Lc<Unit, AppInfoUiState>> {
    override val values: Sequence<Lc<Unit, AppInfoUiState>> =
        sequenceOf(
            Lc.Loading(Unit),
            AppInfoUiState(
                    version = VersionInfo(currentVersion = "2024.9", isSupported = true),
                    isPlayBuild = true,
                )
                .toLc(),
            AppInfoUiState(
                    version = VersionInfo(currentVersion = "2024.9", isSupported = false),
                    isPlayBuild = true,
                )
                .toLc(),
        )
}
