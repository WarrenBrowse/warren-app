package com.warrenbrowse.vpn.receiver.util

/**
 * Whether to bring the tunnel back once the app has been replaced by an update.
 *
 * The update kills the service, and nothing restarts it unless Always-on VPN
 * names this app: a user who was connected is left on the bare network without
 * being told. The desktop clients restore the tunnel after an update; this is
 * the mobile side of it.
 *
 * @param tunnelRequested whether a tunnel was wanted when the update came:
 *   set when the service starts one, cleared when the user disconnects or
 *   another VPN takes over.
 * @param alwaysOnVpn whether the OS ran this app as always-on VPN when its
 *   tunnel last came up. The OS then restarts it by itself, and a
 *   second start would race its own. Recorded by the service because the
 *   system setting is not readable by an app.
 * @param hasVpnPermission whether the VPN permission is still granted; without
 *   it the service cannot open a tunnel, and asking would need the UI.
 */
fun shouldRestoreTunnelAfterUpdate(
    tunnelRequested: Boolean,
    alwaysOnVpn: Boolean,
    hasVpnPermission: Boolean,
): Boolean = tunnelRequested && hasVpnPermission && !alwaysOnVpn
