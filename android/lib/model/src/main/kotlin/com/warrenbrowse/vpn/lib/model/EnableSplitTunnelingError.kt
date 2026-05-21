package com.warrenbrowse.vpn.lib.model

interface EnableSplitTunnelingError {
    data class Unknown(val throwable: Throwable) : EnableSplitTunnelingError
}
