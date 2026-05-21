package com.warrenbrowse.vpn.lib.model

sealed interface GetAccountDataError {
    data class Unknown(val error: Throwable) : GetAccountDataError
}
