package com.warrenbrowse.vpn.lib.model

sealed interface UpdateRelayLocationsError {
    data class Unknown(val throwable: Throwable) : UpdateRelayLocationsError
}
