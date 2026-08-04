package com.warrenbrowse.vpn.lib.model

import android.os.Parcelable
import kotlinx.parcelize.Parcelize

@Parcelize
enum class TransportProtocol : Parcelable {
    Tcp,
    Udp,
}
