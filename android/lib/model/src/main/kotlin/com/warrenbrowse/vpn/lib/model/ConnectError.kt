package com.warrenbrowse.vpn.lib.model

sealed interface ConnectError {
    data class Unknown(val throwable: Throwable) : ConnectError

    data class NotPrepared(val error: PrepareError) : ConnectError
}
