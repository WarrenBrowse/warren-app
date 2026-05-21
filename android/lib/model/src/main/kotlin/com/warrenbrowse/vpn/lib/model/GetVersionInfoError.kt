package com.warrenbrowse.vpn.lib.model

sealed interface GetVersionInfoError {
    data class Unknown(val error: Throwable) : GetVersionInfoError
}
