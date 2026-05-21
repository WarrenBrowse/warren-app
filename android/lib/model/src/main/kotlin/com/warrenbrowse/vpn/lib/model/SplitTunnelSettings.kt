package com.warrenbrowse.vpn.lib.model

data class SplitTunnelSettings(val enabled: Boolean, val excludedApps: Set<PackageName>)
