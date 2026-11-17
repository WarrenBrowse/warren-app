package com.warrenbrowse.vpn.feature.appinfo.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2

// `version` is the installed version, whose source the screen links to.
@Parcelize data class LicensesNavKey(val version: String) : NavKey2
