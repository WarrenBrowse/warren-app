package com.warrenbrowse.vpn.lib.repository

/**
 * Whether to advise turning on the system's own block: Always-on VPN with "Block connections
 * without VPN". It is the only mechanism that keeps traffic blocked while the app is being updated
 * or after its process died, which no app can do for itself. Always-on alone is not enough: it
 * restarts the VPN but lets traffic out while the tunnel is down.
 */
fun alwaysOnVpnAdviceDue(alwaysOnVpn: Boolean, lockdown: Boolean, dismissed: Boolean): Boolean =
    !dismissed && !(alwaysOnVpn && lockdown)
