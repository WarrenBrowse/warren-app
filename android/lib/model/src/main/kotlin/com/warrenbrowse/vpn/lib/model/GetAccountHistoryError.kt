package com.warrenbrowse.vpn.lib.model

sealed interface GetAccountHistoryError {
    data class Unknown(val error: Throwable) : GetAccountHistoryError
}
