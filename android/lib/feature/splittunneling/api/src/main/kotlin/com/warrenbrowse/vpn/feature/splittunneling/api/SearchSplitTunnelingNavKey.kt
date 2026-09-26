package com.warrenbrowse.vpn.feature.splittunneling.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2

/** Searches the list of "VPN only for" when [includeOnly], of "Bypass VPN" otherwise. */
@Parcelize data class SearchSplitTunnelingNavKey(val includeOnly: Boolean = false) : NavKey2
