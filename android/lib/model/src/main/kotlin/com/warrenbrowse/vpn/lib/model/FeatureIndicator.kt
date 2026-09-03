package com.warrenbrowse.vpn.lib.model

// The order of the variants match the priority order and can be sorted on.
enum class FeatureIndicator {
    // The live path measured below the TUN MTU (desktop `reducedMtu`, fed by
    // the negotiated endpoint). A warning about the network, ranked first as on
    // desktop; the user's own MTU setting is CUSTOM_MTU.
    REDUCED_MTU,
    DAITA,
    DAITA_MULTIHOP,
    QUANTUM_RESISTANCE,
    MULTIHOP,
    PORT_FORWARDING,
    SPLIT_TUNNELING,
    UDP_2_TCP,
    SHADOWSOCKS,
    QUIC,
    LWO,
    LAN_SHARING,
    DNS_CONTENT_BLOCKERS,
    CUSTOM_DNS,
    SERVER_IP_OVERRIDE,
    CUSTOM_MTU,
}
