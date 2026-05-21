package com.warrenbrowse.vpn.feature.apiaccess.impl.screen.list

import com.warrenbrowse.vpn.lib.model.ApiAccessMethodSetting

data class ApiAccessListUiState(
    val currentApiAccessMethodSetting: ApiAccessMethodSetting? = null,
    val apiAccessMethodSettings: List<ApiAccessMethodSetting> = emptyList(),
)
