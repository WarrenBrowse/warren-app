package com.warrenbrowse.vpn.feature.vpnsettings.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.NavResult
import com.warrenbrowse.vpn.lib.model.Mtu

@Parcelize data class MtuNavKey(val initialMtu: Mtu? = null) : NavKey2

@Parcelize data class MtuNavResult(val complete: Boolean) : NavResult
