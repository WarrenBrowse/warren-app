package com.warrenbrowse.vpn.feature.splittunneling.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2

/**
 * Opens App routing. [countryPerApp] says it was opened from the connect screen's "apps in other
 * countries" badge, whose shared transition it continues.
 */
@Parcelize
data class SplitTunnelingNavKey(val isModal: Boolean = false, val countryPerApp: Boolean = false) :
    NavKey2
