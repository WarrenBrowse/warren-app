package com.warrenbrowse.vpn.feature.splittunneling.impl

import androidx.compose.ui.tooling.preview.PreviewParameterProvider
import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.AppData
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.toLc
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.ui.resource.R

class SplitTunnelingUiStatePreviewParameterProvider :
    PreviewParameterProvider<Lc<Loading, SplitTunnelingUiState>> {
    override val values =
        sequenceOf(
            SplitTunnelingUiState(
                    enabled = true,
                    excludedApps = excludedApps,
                    includedApps = includedApps,
                    showSystemApps = true,
                )
                .toLc(),
            SplitTunnelingUiState(
                    enabled = true,
                    excludedApps = excludedApps,
                    includedApps = includedApps.filter { !it.isSystemApp },
                    showSystemApps = false,
                )
                .toLc(),
            Lc.Loading(Loading()),
        )
}

private val excludedApps =
    listOf(
        AppData(
            packageName = PackageName("my.package.a"),
            name = "TitleA",
            iconRes = R.drawable.icon_android,
        ),
        AppData(
            packageName = PackageName("my.package.b"),
            name = "TitleB",
            iconRes = R.drawable.icon_android,
        ),
        AppData(
            packageName = PackageName("my.package.c"),
            name = "TitleC (System app)",
            iconRes = R.drawable.icon_android,
            isSystemApp = true,
        ),
    )
private val includedApps =
    listOf(
        AppData(
            packageName = PackageName("my.package.d"),
            name = "TitleD",
            iconRes = R.drawable.icon_android,
        ),
        AppData(
            packageName = PackageName("my.package.e"),
            name = "TitleE (System app)",
            iconRes = R.drawable.icon_android,
            isSystemApp = true,
        ),
    )
