package com.warrenbrowse.vpn.feature.customlist.impl.screen.editname

import com.warrenbrowse.vpn.lib.usecase.customlists.RenameError

data class EditCustomListNameUiState(val name: String = "", val error: RenameError? = null) {
    val isValidName = name.isNotBlank()
}
