package com.warrenbrowse.vpn.lib.usecase

import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import com.warrenbrowse.vpn.lib.common.util.isDaitaAndDirectOnly
import com.warrenbrowse.vpn.lib.common.util.isLwoEnabled
import com.warrenbrowse.vpn.lib.common.util.isQuicEnabled
import com.warrenbrowse.vpn.lib.common.util.shouldFilterByDaita
import com.warrenbrowse.vpn.lib.common.util.shouldFilterByLwo
import com.warrenbrowse.vpn.lib.common.util.shouldFilterByQuic
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.Ownership
import com.warrenbrowse.vpn.lib.model.ProviderId
import com.warrenbrowse.vpn.lib.model.Providers
import com.warrenbrowse.vpn.lib.model.RelayListType
import com.warrenbrowse.vpn.lib.model.Settings
import com.warrenbrowse.vpn.lib.repository.RelayListFilterRepository
import com.warrenbrowse.vpn.lib.repository.SettingsRepository

typealias ModelOwnership = Ownership

class FilterChipUseCase(
    private val relayListFilterRepository: RelayListFilterRepository,
    private val providerToOwnershipsUseCase: ProviderToOwnershipsUseCase,
    private val settingsRepository: SettingsRepository,
) {
    operator fun invoke(relayListType: RelayListType): Flow<List<FilterChip>> =
        combine(
            relayListFilterRepository.selectedOwnership,
            relayListFilterRepository.selectedProviders,
            providerToOwnershipsUseCase(),
            settingsRepository.settingsUpdates,
        ) { selectedOwnership, selectedConstraintProviders, providerOwnership, settings ->
            filterChips(
                selectedOwnership = selectedOwnership,
                selectedConstraintProviders = selectedConstraintProviders,
                providerToOwnerships = providerOwnership,
                settings = settings,
                relayListType = relayListType,
            )
        }

    private fun filterChips(
        selectedOwnership: Constraint<Ownership>,
        selectedConstraintProviders: Constraint<Providers>,
        providerToOwnerships: Map<ProviderId, Set<Ownership>>,
        settings: Settings?,
        relayListType: RelayListType,
    ): List<FilterChip> {
        val ownershipFilter = selectedOwnership.getOrNull()
        val providerCountFilter =
            when (selectedConstraintProviders) {
                is Constraint.Any -> null
                is Constraint.Only ->
                    selectedConstraintProviders.value
                        .filter { providerId ->
                            if (ownershipFilter == null) {
                                true
                            } else {
                                val providerOwnerships = providerToOwnerships[providerId]
                                // If the provider has been removed from the relay list we add it
                                // so it is visible for the user, because we won't know what
                                // ownerships it had.
                                providerOwnerships?.contains(ownershipFilter) ?: true
                            }
                        }
                        .size
            }
        return buildList {
            if (ownershipFilter != null) {
                add(FilterChip.Ownership(ownershipFilter))
            }
            if (providerCountFilter != null) {
                add(FilterChip.Provider(providerCountFilter))
            }
            if (
                shouldFilterByDaita(
                    daitaDirectOnly = settings?.isDaitaAndDirectOnly() == true,
                    relayListType = relayListType,
                )
            ) {
                add(FilterChip.Daita)
            }
            if (
                shouldFilterByQuic(settings?.isQuicEnabled() == true, relayListType = relayListType)
            ) {
                add(FilterChip.Quic)
            }
            if (
                shouldFilterByLwo(settings?.isLwoEnabled() == true, relayListType = relayListType)
            ) {
                add(FilterChip.Lwo)
            }
        }
    }
}

sealed interface FilterChip {
    data class Ownership(val ownership: ModelOwnership) : FilterChip

    data class Provider(val count: Int) : FilterChip

    data object Daita : FilterChip

    data object Entry : FilterChip

    data object Exit : FilterChip

    data object Quic : FilterChip

    data object Lwo : FilterChip
}
