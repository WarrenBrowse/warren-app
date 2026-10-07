package com.warrenbrowse.vpn.feature.splittunneling.impl

import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.AppData
import com.warrenbrowse.vpn.feature.splittunneling.impl.countries.CountryPickerActions
import com.warrenbrowse.vpn.lib.model.AppRoute
import com.warrenbrowse.vpn.lib.model.DefaultRoute

/** Everything the pages of App routing can ask of the view model. */
@Suppress("LongParameterList")
class AppRoutingActions(
    val onChooseDefault: (DefaultRoute) -> Unit,
    val onOpenAddApp: () -> Unit,
    val onAddAppSearchChange: (String) -> Unit,
    val onShowSystemApps: (Boolean) -> Unit,
    val onOpenApp: (AppData) -> Unit,
    val onChooseRoute: (AppRoute) -> Unit,
    /** "Never without the VPN" on the route of the open app. */
    val onSetLocked: (Boolean) -> Unit,
    val onOpenCountries: () -> Unit,
    val onRemoveRule: () -> Unit,
    val onDone: () -> Unit,
    /** One page back, leaving the screen from the list. */
    val onBack: () -> Unit,
    val picker: CountryPickerActions,
    val onConfirmNarrowing: () -> Unit,
    val onCancelNarrowing: () -> Unit,
)
