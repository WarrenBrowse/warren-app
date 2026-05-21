package com.warrenbrowse.vpn.feature.serveripoverride.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.NavResult

@Parcelize data class ImportOverridesNavKey(val overridesActive: Boolean) : NavKey2

@Parcelize data object ImportOverrideByFileNavResult : NavResult
