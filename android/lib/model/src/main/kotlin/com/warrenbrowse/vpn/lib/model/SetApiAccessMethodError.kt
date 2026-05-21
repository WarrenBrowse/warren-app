package com.warrenbrowse.vpn.lib.model

sealed interface SetApiAccessMethodError {
    data class Unknown(val t: Throwable) : SetApiAccessMethodError
}
