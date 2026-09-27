package com.warrenbrowse.vpn.feature.splittunneling.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2

/**
 * Opens App routing on the tab of the split mode in force, or on "Country per app" when
 * [countryPerApp].
 */
@Parcelize
data class SplitTunnelingNavKey(val isModal: Boolean = false, val countryPerApp: Boolean = false) :
    NavKey2
