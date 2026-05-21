package com.warrenbrowse.vpn.lib.model

sealed interface GetCurrentApiAccessMethodError {
    data class Unknown(val t: Throwable) : GetCurrentApiAccessMethodError
}
