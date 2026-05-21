package com.warrenbrowse.vpn.lib.model

data class WireguardEndpointData(
    val portRanges: List<PortRange>,
    val shadowsocksPortRanges: List<PortRange>,
)
