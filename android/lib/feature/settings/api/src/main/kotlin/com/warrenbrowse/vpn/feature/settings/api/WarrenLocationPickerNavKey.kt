package com.warrenbrowse.vpn.feature.settings.api

import com.warrenbrowse.vpn.core.NavKey2
import kotlinx.parcelize.Parcelize

/**
 * Navigation key for the Warren location picker screen. Reached
 * from the Warren tunnel settings ("Location" entry). Writes the
 * user's selection to `WarrenLocalSettingsRepository.selectedExitId`.
 */
@Parcelize data object WarrenLocationPickerNavKey : NavKey2
