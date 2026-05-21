package com.warrenbrowse.vpn.feature.apiaccess.impl.screen.list

import androidx.compose.ui.tooling.preview.PreviewParameterProvider
import com.warrenbrowse.vpn.feature.apiaccess.impl.defaultAccessMethods
import com.warrenbrowse.vpn.feature.apiaccess.impl.shadowsocks
import com.warrenbrowse.vpn.feature.apiaccess.impl.socks5Remote

class ApiAccessListUiStatePreviewParameterProvider :
    PreviewParameterProvider<ApiAccessListUiState> {

    override val values: Sequence<ApiAccessListUiState> =
        sequenceOf(
            // Default state
            ApiAccessListUiState(),
            // Without custom api access method
            ApiAccessListUiState(
                currentApiAccessMethodSetting = defaultAccessMethods.first(),
                apiAccessMethodSettings = defaultAccessMethods,
            ),
            // With custom api
            ApiAccessListUiState(
                currentApiAccessMethodSetting = defaultAccessMethods.first(),
                apiAccessMethodSettings =
                    defaultAccessMethods.plus(listOf(shadowsocks, socks5Remote)),
            ),
        )
}
