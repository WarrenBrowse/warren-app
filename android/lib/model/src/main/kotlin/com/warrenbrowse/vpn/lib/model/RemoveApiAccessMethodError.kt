package com.warrenbrowse.vpn.lib.model

sealed interface RemoveApiAccessMethodError {
    data class Unknown(val t: Throwable) : RemoveApiAccessMethodError
}
