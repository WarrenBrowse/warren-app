package com.warrenbrowse.vpn.lib.model

interface ClearAccountHistoryError {
    data class Unknown(val t: Throwable) : ClearAccountHistoryError
}
