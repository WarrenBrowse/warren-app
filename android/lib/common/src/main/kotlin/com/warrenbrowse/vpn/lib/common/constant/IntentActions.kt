package com.warrenbrowse.vpn.lib.common.constant

// Actions
const val KEY_CONNECT_ACTION = "$WARREN_PACKAGE_NAME.connect_action"
const val KEY_RECONNECT_ACTION = "$WARREN_PACKAGE_NAME.reconnect_action"
const val KEY_DISCONNECT_ACTION = "$WARREN_PACKAGE_NAME.disconnect_action"
const val KEY_REQUEST_VPN_PROFILE = "$WARREN_PACKAGE_NAME.request_vpn_profile"

/**
 * D.4 step 7: dedicated Quinn connect action. Carries a serialised
 * [com.warrenbrowse.vpn.app.service.WarrenTunnelConfig] under the
 * [KEY_WARREN_TUNNEL_CONFIG_JSON] extra. The service deserialises the
 * config, retrieves the wallet mnemonic via the in-process
 * `WalletRepository`, and dispatches to `WarrenQuinnAdapter.connect`.
 *
 * Coexists with [KEY_CONNECT_ACTION] (which still routes through the
 * dead legacy `ConnectionProxy` for now) so the migration is
 * incremental. Once every caller has migrated, the legacy action is
 * dropped.
 */
const val KEY_WARREN_CONNECT_QUINN_ACTION = "$WARREN_PACKAGE_NAME.warren_connect_quinn_action"

/** JSON-serialised `WarrenTunnelConfig` carried with [KEY_WARREN_CONNECT_QUINN_ACTION]. */
const val KEY_WARREN_TUNNEL_CONFIG_JSON = "$WARREN_PACKAGE_NAME.warren_tunnel_config_json"
