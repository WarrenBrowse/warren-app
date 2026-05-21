package com.warrenbrowse.vpn.lib.model

interface ListDevicesError {
    data class Unknown(val throwable: Throwable) : ListDevicesError
}
