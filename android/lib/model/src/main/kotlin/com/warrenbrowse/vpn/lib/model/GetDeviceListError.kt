package com.warrenbrowse.vpn.lib.model

sealed interface GetDeviceListError {
    data class Unknown(val error: Throwable) : GetDeviceListError
}
