package com.warrenbrowse.vpn.lib.model

interface LogoutAccountError {
    data class Unknown(val t: Throwable) : LogoutAccountError
}
