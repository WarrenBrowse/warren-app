package com.warrenbrowse.vpn.lib.model

data class RelayList(
    val countries: List<RelayItem.Location.Country>,
    val wireguardEndpointData: WireguardEndpointData,
)
