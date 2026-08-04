package com.warrenbrowse.vpn.lib.model

sealed interface SetRelayLocationError {
    data class Unknown(val throwable: Throwable) : SetRelayLocationError
}
