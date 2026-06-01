package com.warrenbrowse.vpn.lib.usecase

import arrow.core.raise.nullable
import kotlinx.coroutines.flow.combine
import com.warrenbrowse.vpn.lib.common.util.relaylist.findCity
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.CustomListId
import com.warrenbrowse.vpn.lib.model.GeoLocationId
import com.warrenbrowse.vpn.lib.model.RelayItem
import com.warrenbrowse.vpn.lib.model.RelayItemId
import com.warrenbrowse.vpn.lib.repository.RelayListRepository

// Resolves the title for the selected location. A stale persisted CustomListId
// returns null, so no title is rendered and ConnectViewModel falls back to the
// default "Switch location" placeholder.
class SelectedLocationTitleUseCase(
    private val relayListRepository: RelayListRepository,
) {
    operator fun invoke() =
        combine(
            relayListRepository.relayList,
            relayListRepository.selectedLocation,
        ) { relayList, selectedLocation ->
            if (selectedLocation is Constraint.Only) {
                createRelayItemTitle(selectedLocation.value, relayList)
            } else {
                null
            }
        }

    private fun createRelayItemTitle(
        relayItemId: RelayItemId,
        relayCountries: List<RelayItem.Location.Country>,
    ): String? =
        when (relayItemId) {
            is CustomListId -> null
            is GeoLocationId.Hostname -> createRelayTitle(relayCountries, relayItemId)
            is GeoLocationId.City -> relayCountries.findCity(relayItemId)?.name
            is GeoLocationId.Country -> relayCountries.firstOrNull { it.id == relayItemId }?.name
        }

    private fun createRelayTitle(
        relayCountries: List<RelayItem.Location.Country>,
        relayItemId: GeoLocationId.Hostname,
    ): String? = nullable {
        val city = relayCountries.findCity(relayItemId.city).bind()
        val relay = city.relays.find { it.id == relayItemId }.bind()

        relay.formatTitle(city)
    }

    private fun RelayItem.Location.Relay.formatTitle(city: RelayItem.Location.City) =
        "${city.name} (${name})"
}
