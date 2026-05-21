package com.warrenbrowse.vpn.lib.model

sealed interface SetWireguardMtuError {
    data class Unknown(val throwable: Throwable) : SetWireguardMtuError
}
