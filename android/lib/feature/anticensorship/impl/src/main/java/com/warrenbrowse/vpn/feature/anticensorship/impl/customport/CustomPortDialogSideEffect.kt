package com.warrenbrowse.vpn.feature.anticensorship.impl.customport

import com.warrenbrowse.vpn.lib.model.Port

sealed interface CustomPortDialogSideEffect {
    data class Success(val port: Port?) : CustomPortDialogSideEffect
}
