package com.warrenbrowse.vpn.lib.common.constant

// Do not use in cases where the application id is expected since the application id will differ
// between different builds.
internal const val MULLVAD_PACKAGE_NAME = "com.warrenbrowse.vpn"

// Classes
const val MAIN_ACTIVITY_CLASS = "$MULLVAD_PACKAGE_NAME.app.MainActivity"
const val VPN_SERVICE_CLASS = "$MULLVAD_PACKAGE_NAME.app.service.WarrenVpnService"
