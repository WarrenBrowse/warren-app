package com.warrenbrowse.vpn.lib.model

sealed interface SetDnsOptionsError {
    data class Unknown(val throwable: Throwable) : SetDnsOptionsError
}
