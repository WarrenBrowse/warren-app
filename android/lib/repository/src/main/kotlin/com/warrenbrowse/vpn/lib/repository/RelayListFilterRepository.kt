package com.warrenbrowse.vpn.lib.repository

import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import com.warrenbrowse.vpn.lib.grpc.ManagementService
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.Ownership
import com.warrenbrowse.vpn.lib.model.Providers

class RelayListFilterRepository(
    private val managementService: ManagementService,
    dispatcher: CoroutineDispatcher = Dispatchers.IO,
) {
    val selectedOwnership: StateFlow<Constraint<Ownership>> =
        managementService.settings
            .map { settings -> settings.relaySettings.relayConstraints.ownership }
            .stateIn(CoroutineScope(dispatcher), SharingStarted.WhileSubscribed(), Constraint.Any)

    val selectedProviders: StateFlow<Constraint<Providers>> =
        managementService.settings
            .map { settings -> settings.relaySettings.relayConstraints.providers }
            .stateIn(CoroutineScope(dispatcher), SharingStarted.WhileSubscribed(), Constraint.Any)

    suspend fun updateSelectedOwnershipAndProviderFilter(
        ownership: Constraint<Ownership>,
        providers: Constraint<Providers>,
    ) = managementService.setOwnershipAndProviders(ownership, providers)

    suspend fun updateSelectedOwnership(value: Constraint<Ownership>) =
        managementService.setOwnership(value)

    suspend fun updateSelectedProviders(value: Constraint<Providers>) =
        managementService.setProviders(value)
}
