package com.warrenbrowse.vpn.feature.settings.api

import com.warrenbrowse.vpn.core.NavResult
import kotlinx.parcelize.Parcelize

/**
 * Handed back by the location picker when a pick happened with no tunnel up:
 * on desktop choosing a location IS the connect gesture, so the caller starts
 * the tunnel on return. The picker cannot dispatch it itself because the VPN
 * consent gate and the biometric host belong to the calling screen.
 */
@Parcelize data object ConnectAfterLocationPick : NavResult
