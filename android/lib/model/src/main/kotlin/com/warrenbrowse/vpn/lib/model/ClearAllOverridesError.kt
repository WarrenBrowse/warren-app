package com.warrenbrowse.vpn.lib.model

sealed interface ClearAllOverridesError {
    data class Unknown(val throwable: Throwable) : ClearAllOverridesError
}
