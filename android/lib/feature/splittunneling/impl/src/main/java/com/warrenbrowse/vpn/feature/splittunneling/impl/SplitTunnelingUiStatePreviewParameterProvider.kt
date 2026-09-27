package com.warrenbrowse.vpn.feature.splittunneling.impl

import androidx.compose.ui.tooling.preview.PreviewParameterProvider
import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.AppData
import com.warrenbrowse.vpn.feature.splittunneling.impl.countries.AppCountryItem
import com.warrenbrowse.vpn.feature.splittunneling.impl.countries.CountryPerAppUiState
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.toLc
import com.warrenbrowse.vpn.lib.model.AppExit
import com.warrenbrowse.vpn.lib.model.AppRouteLine
import com.warrenbrowse.vpn.lib.model.AppRouteUnavailableReason
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.model.SplitTunnelMode
import com.warrenbrowse.vpn.lib.ui.resource.R

class SplitTunnelingUiStatePreviewParameterProvider :
    PreviewParameterProvider<Lc<Loading, SplitTunnelingUiState>> {
    override val values =
        sequenceOf(
            SplitTunnelingUiState(
                    splitMode = SplitTunnelMode.Exclude,
                    selectedApps = excludedApps,
                    otherApps = includedApps,
                    showSystemApps = true,
                )
                .toLc(),
            SplitTunnelingUiState(
                    splitMode = SplitTunnelMode.IncludeOnly,
                    tab = SplitTunnelingTab.IncludeOnly,
                    selectedApps = excludedApps,
                    otherApps = includedApps.filter { !it.isSystemApp },
                    showSystemApps = false,
                )
                .toLc(),
            SplitTunnelingUiState(
                    tab = SplitTunnelingTab.CountryPerApp,
                    countryPerApp =
                        CountryPerAppUiState(
                            enabled = true,
                            withCountry =
                                listOf(
                                    AppCountryItem(
                                        excludedApps[0],
                                        AppExit("se"),
                                        AppRouteLine.Connected("198.51.100.7"),
                                    ),
                                    AppCountryItem(
                                        excludedApps[1],
                                        AppExit("de", "Berlin"),
                                        AppRouteLine.Unavailable(
                                            AppRouteUnavailableReason.WaitingForRoute
                                        ),
                                    ),
                                ),
                            otherApps = includedApps,
                        ),
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
