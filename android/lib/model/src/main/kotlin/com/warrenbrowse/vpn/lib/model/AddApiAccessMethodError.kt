package com.warrenbrowse.vpn.lib.model

sealed interface AddApiAccessMethodError {
    data class Unknown(val t: Throwable) : AddApiAccessMethodError
}
