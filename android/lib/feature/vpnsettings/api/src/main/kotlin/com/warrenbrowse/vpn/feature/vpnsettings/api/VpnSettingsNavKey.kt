package com.warrenbrowse.vpn.feature.vpnsettings.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.lib.model.FeatureIndicator

@Parcelize
data class VpnSettingsNavKey(
    val scrollToFeature: FeatureIndicator? = null,
    val isModal: Boolean = false,
) : NavKey2
