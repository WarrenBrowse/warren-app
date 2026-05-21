package com.warrenbrowse.vpn.feature.customlist.impl.screen.create

import com.warrenbrowse.vpn.lib.usecase.customlists.CreateWithLocationsError

data class CreateCustomListUiState(val error: CreateWithLocationsError? = null)
