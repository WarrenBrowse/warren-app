package com.warrenbrowse.vpn.feature.customlist.impl.screen.delete

import com.warrenbrowse.vpn.lib.model.CustomListName
import com.warrenbrowse.vpn.lib.usecase.customlists.DeleteWithUndoError

data class DeleteCustomListUiState(val name: CustomListName, val deleteError: DeleteWithUndoError?)
