package com.warrenbrowse.vpn.feature.anticensorship.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.lib.model.FeatureIndicator

@Parcelize
data class AntiCensorshipNavKey(
    val selectedFeature: FeatureIndicator? = null,
    val isModal: Boolean = false,
) : NavKey2
