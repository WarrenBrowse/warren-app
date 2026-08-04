package com.warrenbrowse.vpn.lib.model

import java.net.InetSocketAddress

data class Endpoint(val address: InetSocketAddress, val protocol: TransportProtocol)
