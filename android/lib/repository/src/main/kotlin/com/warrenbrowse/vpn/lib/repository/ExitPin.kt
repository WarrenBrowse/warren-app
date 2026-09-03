package com.warrenbrowse.vpn.lib.repository

/**
 * What the user pinned in the location picker.
 *
 * The desktop expresses a location as `{country}`, `{country, city}` or
 * `{country, city, hostname}`, so every geographical level is a first-class
 * target; this carries the same three depths plus the explicit "nothing
 * pinned" case that the picker surfaces as its Automatic row.
 */
sealed interface ExitPin {
    /** No pin: the connect path applies its own fallback (fastest/first active). */
    data object Automatic : ExitPin

    /** Any active exit in [country] (ISO alpha-2, matched case-insensitively). */
    data class Country(val country: String) : ExitPin

    /** Any active exit in [city] of [country], both matched case-insensitively. */
    data class City(val country: String, val city: String) : ExitPin

    /** One specific exit, by its stable 16-byte exit id. */
    data class Exit(val exitId: String) : ExitPin
}

/**
 * A display name already resolved for one exit, cached so a cold start can
 * name the pinned exit before the relay catalogue has been fetched. It carries
 * [exitId] so a label resolved for a previous selection is never shown for the
 * current one.
 */
data class ExitPinLabel(val exitId: String, val label: String)

/**
 * Resolve [pin] to the one concrete exit the engine will dial, or `null` when
 * the pin names nothing usable (an empty scope, an inactive exit, or
 * [ExitPin.Automatic], which pins nothing on purpose so the caller's own
 * fallback chain still runs).
 *
 * Candidates are the ACTIVE exits inside the pinned scope, the same rule the
 * shuffle button applies. The pick among them is the heaviest, tie-broken by
 * exit id, so re-dialling the same country lands on the same exit instead of
 * hopping between nodes on every reconnect.
 */
fun resolveExitPin(pin: ExitPin, relays: List<WarrenRelaySummary>): WarrenRelaySummary? {
    val candidates = when (pin) {
        ExitPin.Automatic -> return null
        is ExitPin.Exit -> relays.filter { it.active && it.exitId == pin.exitId }
        is ExitPin.City ->
            relays.filter {
                it.active &&
                    it.country.equals(pin.country, ignoreCase = true) &&
                    it.city.equals(pin.city, ignoreCase = true)
            }
        is ExitPin.Country ->
            relays.filter { it.active && it.country.equals(pin.country, ignoreCase = true) }
    }
    return heaviest(candidates)
}

/**
 * The exit an automatic retry dials once [failedExitPubkeyHex] dropped, or
 * `null` when the pin leaves no alternative and the same exit is retried.
 *
 * The desktop selector's failover policy
 * (`WarrenRelaySelector::select_failover_alternative`): an active alternative
 * inside the pinned scope, in the failed exit's own country first, then in any
 * country the scope allows, and never the failed exit itself. An automatic pin
 * is scoped by [exitCountry] when one is preferred. The pick among the
 * candidates is the [resolveExitPin] rule, so the same drop lands on the same
 * alternative every time instead of hopping between spares.
 */
fun resolveFailoverExit(
    pin: ExitPin,
    exitCountry: String?,
    relays: List<WarrenRelaySummary>,
    failedExitPubkeyHex: String,
): WarrenRelaySummary? {
    val inScope =
        relays.filter { it.active && it.exitPubkeyHex != failedExitPubkeyHex && pin.admits(it, exitCountry) }
    val failedCountry = relays.firstOrNull { it.exitPubkeyHex == failedExitPubkeyHex }?.country
    val sameCountry =
        if (failedCountry == null) emptyList()
        else inScope.filter { it.country.equals(failedCountry, ignoreCase = true) }
    return heaviest(sameCountry.ifEmpty { inScope })
}

private fun ExitPin.admits(relay: WarrenRelaySummary, exitCountry: String?): Boolean =
    when (this) {
        ExitPin.Automatic ->
            exitCountry.isNullOrBlank() || relay.country.equals(exitCountry, ignoreCase = true)
        is ExitPin.Exit -> relay.exitId == exitId
        is ExitPin.City ->
            relay.country.equals(country, ignoreCase = true) &&
                relay.city.equals(city, ignoreCase = true)
        is ExitPin.Country -> relay.country.equals(country, ignoreCase = true)
    }

private fun heaviest(candidates: List<WarrenRelaySummary>): WarrenRelaySummary? =
    candidates.maxWithOrNull(compareBy({ it.weight }, { it.exitId }))

/** True when [country] is exactly what is pinned, with no deeper selection. */
fun ExitPin.pinsCountry(country: String): Boolean =
    this is ExitPin.Country && this.country.equals(country, ignoreCase = true)

/** True when [city] in [country] is exactly what is pinned. */
fun ExitPin.pinsCity(country: String, city: String): Boolean =
    this is ExitPin.City &&
        this.country.equals(country, ignoreCase = true) &&
        this.city.equals(city, ignoreCase = true)
