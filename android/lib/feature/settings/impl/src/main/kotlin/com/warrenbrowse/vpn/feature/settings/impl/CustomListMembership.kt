package com.warrenbrowse.vpn.feature.settings.impl

import com.warrenbrowse.vpn.lib.repository.ExitPin
import com.warrenbrowse.vpn.lib.repository.pinsCity
import com.warrenbrowse.vpn.lib.repository.pinsCountry

/** True when [other] names the same location, codes and city names compared case-insensitively. */
internal fun ExitPin.sameLocationAs(other: ExitPin): Boolean =
    when (this) {
        ExitPin.Automatic -> other == ExitPin.Automatic
        is ExitPin.Country -> other.pinsCountry(country)
        is ExitPin.City -> other.pinsCity(country, city)
        is ExitPin.Exit -> other is ExitPin.Exit && other.exitId == exitId
    }

/** One line of a row's lists menu: a list, checked when it holds the row's location. */
internal data class ListMembership(val name: String, val contains: Boolean)

/**
 * The lists menu of a row standing for [location]: every list, an empty one
 * included so it can be filled again, in name order.
 */
internal fun listMemberships(
    location: ExitPin,
    customLists: Map<String, List<ExitPin>>,
): List<ListMembership> =
    customLists.entries
        .sortedBy { it.key }
        .map { (name, members) -> ListMembership(name, members.any { it.sameLocationAs(location) }) }

/** What tapping a list in the lists menu did. */
internal enum class ListToggle {
    Added,
    Removed,
}

/**
 * Flip [location]'s membership of list [name]. Removal hands [remove] the entry
 * as stored, so a case difference between the catalogue and the stored entry
 * never leaves the location behind.
 */
internal fun toggleListMembership(
    name: String,
    location: ExitPin,
    customLists: Map<String, List<ExitPin>>,
    add: (String, ExitPin) -> Unit,
    remove: (String, ExitPin) -> Unit,
): ListToggle {
    val stored = customLists[name].orEmpty().firstOrNull { it.sameLocationAs(location) }
    return if (stored == null) {
        add(name, location)
        ListToggle.Added
    } else {
        remove(name, stored)
        ListToggle.Removed
    }
}
