package com.warrenbrowse.vpn.lib.model

interface AddSplitTunnelingAppError {
    data class Unknown(val throwable: Throwable) : AddSplitTunnelingAppError
}
