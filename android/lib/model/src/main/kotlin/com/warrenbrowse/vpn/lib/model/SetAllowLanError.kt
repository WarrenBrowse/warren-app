package com.warrenbrowse.vpn.lib.model

sealed interface SetAllowLanError {
    data class Unknown(val throwable: Throwable) : SetAllowLanError
}
