package com.warrenbrowse.vpn.feature.home.impl.connect

import com.warrenbrowse.vpn.lib.model.Endpoint
import com.warrenbrowse.vpn.lib.model.GeoIpLocation
import com.warrenbrowse.vpn.lib.model.TunnelEndpoint
import com.warrenbrowse.vpn.lib.repository.ExitPin
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary
import com.warrenbrowse.vpn.lib.repository.label

/**
 * The location the connection card shows for a pin the engine has not resolved
 * into a live exit yet.
 *
 * Automatic names nothing on purpose: the desktop returns no target country for
 * an "any" constraint, so the location line stays empty and the backdrop stays
 * neutral instead of painting an arbitrary catalogue relay's country and then
 * snapping to the real exit when the tunnel comes up.
 *
 * A pin is shown at the depth the user chose, so a country pin never invents a
 * city.
 */
internal fun pinnedExitLocation(pin: ExitPin, relays: List<WarrenRelaySummary>): GeoIpLocation? =
    when (pin) {
        ExitPin.Automatic -> null
        is ExitPin.Country -> geoLocation(pin.country, city = null)
        is ExitPin.City -> geoLocation(pin.country, pin.city.ifBlank { null })
        is ExitPin.Exit ->
            relays.firstOrNull { it.exitId == pin.exitId }
                ?.let { geoLocation(it.country, it.city.ifBlank { null }) }
    }

/**
 * The exit the tunnel is actually dialling or running on, named by its
 * [label]. Matched by the exit key the config carries, which tells apart two
 * exits behind one address, then by endpoint host (a tunnel that names no key,
 * or a key the catalogue no longer lists). Null when nothing matches, so the
 * caller falls back to the pinned scope.
 *
 * Only the exit is named: a multi-hop circuit's entry is never a catalogue
 * exit the user picked, and the desktop names the exit alone too.
 */
internal fun activeExitLocation(
    tunnel: TunnelEndpoint,
    relays: List<WarrenRelaySummary>,
): GeoIpLocation? {
    val key = tunnel.exitPubkeyHex
    val host = tunnel.endpoint.hostLiteral()
    val exit =
        key?.let { relays.firstOrNull { relay -> relay.exitPubkeyHex.equals(it, ignoreCase = true) } }
            ?: host?.let { relays.firstOrNull { relay -> relay.endpoint.substringBeforeLast(':') == it } }
    return exit?.let { geoLocation(it.country, it.city.ifBlank { null })?.copy(hostname = it.label) }
}

/** The endpoint host as the relay catalogue spells it (an IP literal). */
internal fun Endpoint.hostLiteral(): String? = address.address?.hostAddress ?: address.hostString

private fun geoLocation(country: String, city: String?): GeoIpLocation? {
    if (country.isBlank()) return null
    // Coordinates no longer drive anything on Android (the GL map gave way to
    // the scenery backdrop), so a country with no centroid must still yield a
    // label rather than erase the exit the engine did name.
    val (latitude, longitude) = CountryCentroid.of(country) ?: (0.0 to 0.0)
    return GeoIpLocation(
        ipv4 = null,
        ipv6 = null,
        country = country,
        city = city,
        latitude = latitude,
        longitude = longitude,
        hostname = null,
        entryHostname = null,
    )
}
