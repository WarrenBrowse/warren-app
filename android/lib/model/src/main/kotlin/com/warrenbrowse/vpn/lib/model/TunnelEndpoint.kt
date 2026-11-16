package com.warrenbrowse.vpn.lib.model

data class TunnelEndpoint(
    val entryEndpoint: Endpoint?,
    val endpoint: Endpoint,
    val quantumResistant: Boolean,
    val obfuscation: ObfuscationEndpoint?,
    val daita: Boolean,
    // The exit's key, which names the exit the tunnel runs on even when two
    // catalogue exits share an address. Null when the tunnel was not dialled
    // from a Warren config.
    val exitPubkeyHex: String? = null,
)
