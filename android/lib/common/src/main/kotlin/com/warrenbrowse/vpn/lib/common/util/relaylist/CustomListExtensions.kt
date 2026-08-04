package com.warrenbrowse.vpn.lib.common.util.relaylist

import com.warrenbrowse.vpn.lib.model.CustomList
import com.warrenbrowse.vpn.lib.model.CustomListId
import com.warrenbrowse.vpn.lib.model.RelayItem

fun CustomList.toRelayItemCustomList(
    relayCountries: List<RelayItem.Location.Country>
): RelayItem.CustomList =
    RelayItem.CustomList(
        customList = this,
        locations = locations.mapNotNull { relayCountries.findByGeoLocationId(it) },
    )

fun List<RelayItem.CustomList>.filterOnSearchTerm(searchTerm: String) =
    if (searchTerm.isNotEmpty()) {
        this.filter { it.name.contains(searchTerm, ignoreCase = true) }
    } else {
        this
    }

fun RelayItem.CustomList.canAddLocation(location: RelayItem) =
    this.locations.none { it.id == location.id } &&
        this.locations.flatMap { it.descendants() }.none { it.id == location.id }

fun List<RelayItem.CustomList>.getById(id: CustomListId) = this.find { it.id == id }

fun List<CustomList>.getById(id: CustomListId) = this.find { it.id == id }

fun RelayItem.CustomList.onlyContains(relayItem: RelayItem.Location) =
    this.locations.size == 1 && this.locations.first() == relayItem
