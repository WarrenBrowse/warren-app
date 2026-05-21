package com.warrenbrowse.vpn.lib.repository

import arrow.optics.Every
import arrow.optics.copy
import arrow.optics.dsl.every
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import com.warrenbrowse.vpn.lib.common.util.relaylist.findByGeoLocationId
import com.warrenbrowse.vpn.lib.common.util.relaylist.sortedByName
import com.warrenbrowse.vpn.lib.grpc.ManagementService
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.GeoLocationId
import com.warrenbrowse.vpn.lib.model.PortRange
import com.warrenbrowse.vpn.lib.model.RelayItem
import com.warrenbrowse.vpn.lib.model.RelayItemId
import com.warrenbrowse.vpn.lib.model.WireguardEndpointData
import com.warrenbrowse.vpn.lib.model.cities
import com.warrenbrowse.vpn.lib.model.cityName
import com.warrenbrowse.vpn.lib.model.countryName
import com.warrenbrowse.vpn.lib.model.name
import com.warrenbrowse.vpn.lib.model.relays

class RelayListRepository(
    private val managementService: ManagementService,
    translationRepository: RelayLocationTranslationRepository,
    dispatcher: CoroutineDispatcher = Dispatchers.IO,
) {
    val relayList: StateFlow<List<RelayItem.Location.Country>> =
        combine(managementService.relayCountries, translationRepository.translations) {
                countries,
                translations ->
                countries.translateRelays(translations)
            }
            .stateIn(CoroutineScope(dispatcher), SharingStarted.WhileSubscribed(), emptyList())

    private fun List<RelayItem.Location.Country>.translateRelays(
        translations: Translations
    ): List<RelayItem.Location.Country> {
        if (translations.isEmpty()) {
            return this
        }

        return Every.list<RelayItem.Location.Country>()
            .modify(this) { country ->
                country.copy {
                    RelayItem.Location.Country.name set translations.lookup(country.name)

                    val cityTraversal = RelayItem.Location.Country.cities.every(Every.list())

                    cityTraversal.name transform { translations.lookup(it) }
                    cityTraversal.countryName transform { translations.lookup(it) }

                    val relayTraversal = cityTraversal.relays.every(Every.list())

                    relayTraversal.cityName transform { translations.lookup(it) }
                    relayTraversal.countryName transform { translations.lookup(it) }

                    RelayItem.Location.Country.cities transform { cities -> cities.sortedByName() }
                }
            }
            .sortedByName()
    }

    val wireguardEndpointData: StateFlow<WireguardEndpointData> =
        managementService.wireguardEndpointData.stateIn(
            CoroutineScope(dispatcher),
            SharingStarted.WhileSubscribed(),
            defaultWireguardEndpointData(),
        )

    val selectedLocation: StateFlow<Constraint<RelayItemId>> =
        managementService.settings
            .map { it.relaySettings.relayConstraints.location }
            .stateIn(CoroutineScope(dispatcher), SharingStarted.WhileSubscribed(), Constraint.Any)

    val portRanges: Flow<List<PortRange>> =
        wireguardEndpointData.map { it.portRanges }.distinctUntilChanged()

    val shadowsocksPortRanges: Flow<List<PortRange>> =
        wireguardEndpointData.map { it.shadowsocksPortRanges }.distinctUntilChanged()

    suspend fun updateSelectedRelayLocation(value: RelayItemId) =
        managementService.setRelayLocation(value)

    suspend fun updateSelectedRelayLocationMultihop(
        isMultihopEnabled: Boolean,
        entry: RelayItemId,
        exit: RelayItemId,
    ) = managementService.setRelayLocationMultihop(isMultihopEnabled, entry, exit)

    suspend fun updateExitRelayLocationMultihop(isMultihopEnabled: Boolean, exit: RelayItemId) =
        managementService.setRelayLocationMultihop(
            isMultihopEnabled = isMultihopEnabled,
            entry = null,
            exit = exit,
        )

    suspend fun refreshRelayList() = managementService.updateRelayLocations()

    fun find(geoLocationId: GeoLocationId): RelayItem.Location? =
        relayList.value.findByGeoLocationId(geoLocationId)

    private fun defaultWireguardEndpointData() = WireguardEndpointData(emptyList(), emptyList())
}
