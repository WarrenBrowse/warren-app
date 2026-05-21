package com.warrenbrowse.vpn.lib.model

data class TunnelEndpoint(
    val entryEndpoint: Endpoint?,
    val endpoint: Endpoint,
    val quantumResistant: Boolean,
    val obfuscation: ObfuscationEndpoint?,
    val daita: Boolean,
)
