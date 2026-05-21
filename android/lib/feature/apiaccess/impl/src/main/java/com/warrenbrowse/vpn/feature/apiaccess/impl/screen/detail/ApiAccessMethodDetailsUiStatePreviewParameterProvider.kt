package com.warrenbrowse.vpn.feature.apiaccess.impl.screen.detail

import androidx.compose.ui.tooling.preview.PreviewParameterProvider
import com.warrenbrowse.vpn.feature.apiaccess.impl.defaultAccessMethods
import com.warrenbrowse.vpn.feature.apiaccess.impl.shadowsocks
import com.warrenbrowse.vpn.lib.model.ApiAccessMethod
import com.warrenbrowse.vpn.lib.model.ApiAccessMethodSetting
import com.warrenbrowse.vpn.lib.model.Cipher
import com.warrenbrowse.vpn.lib.model.Port

class ApiAccessMethodDetailsUiStatePreviewParameterProvider :
    PreviewParameterProvider<ApiAccessMethodDetailsUiState> {
    override val values: Sequence<ApiAccessMethodDetailsUiState> =
        sequenceOf(
            ApiAccessMethodDetailsUiState.Loading(shadowsocks.id),
            // Non-editable api access type
            defaultAccessMethods[0].let {
                ApiAccessMethodDetailsUiState.Content(
                    apiAccessMethodSetting =
                        ApiAccessMethodSetting(
                            id = it.id,
                            name = it.name,
                            enabled = it.enabled,
                            apiAccessMethod = ApiAccessMethod.Direct,
                        ),
                    isCurrentMethod = false,
                    isDisableable = true,
                    isTestingAccessMethod = false,
                )
            },
            // Editable api access type, current method, can not be disabled
            shadowsocks.let {
                ApiAccessMethodDetailsUiState.Content(
                    apiAccessMethodSetting =
                        ApiAccessMethodSetting(
                            id = it.id,
                            name = it.name,
                            enabled = it.enabled,
                            apiAccessMethod =
                                ApiAccessMethod.CustomProxy.Shadowsocks(
                                    "123.123.123.123",
                                    Port.fromString("1234").getOrNull()!!,
                                    null,
                                    Cipher.CHACHA20_IETF_POLY1305,
                                ),
                        ),
                    isCurrentMethod = true,
                    isDisableable = false,
                    isTestingAccessMethod = false,
                )
            },
        )
}
