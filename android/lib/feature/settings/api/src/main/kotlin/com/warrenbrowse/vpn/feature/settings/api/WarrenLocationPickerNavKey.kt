package com.warrenbrowse.vpn.feature.settings.api

import com.warrenbrowse.vpn.core.NavKey2
import kotlinx.parcelize.Parcelize

/**
 * Navigation key for the Warren location picker screen. Reached from the
 * Connect screen and from Port forwarding. Writes the user's selection to
 * `WarrenLocalSettingsRepository.exitPin` and pops back to its caller.
 *
 * [connectOnPick] is set only by a caller able to start the tunnel itself: a
 * pick with no tunnel up hands a [ConnectAfterLocationPick] result back, and a
 * caller that cannot consume it would leave the result to fire on a later,
 * unrelated screen.
 */
@Parcelize
data class WarrenLocationPickerNavKey(val connectOnPick: Boolean = false) : NavKey2
