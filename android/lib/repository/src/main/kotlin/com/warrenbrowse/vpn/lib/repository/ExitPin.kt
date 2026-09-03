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
 * Resolves a pin, or a drop, to the one concrete exit the engine dials.
 *
 * The rule lives in Rust (`warren-jni/src/exit_pin.rs`, over the shared
 * `warren_discovery_core::pick_exit`): the heaviest active exit inside the
 * pinned scope, ties broken by the smallest exit id, the same choice the
 * desktop daemon makes from the same list. Kotlin only carries the pin and
 * the relay snapshot across the JNI and reads the chosen position back, so
 * no second copy of the rule exists to drift. The production binding is
 * `app/connect/JniExitPinResolver`; JVM tests substitute a scripted one.
 */
interface ExitPinResolver {
    /**
     * The exit [pin] resolves to among [relays], or `null` when the pin names
     * nothing usable (an empty scope, an inactive exit, or [ExitPin.Automatic],
     * which pins nothing on purpose so the caller's own fallback chain runs).
     */
    fun resolve(pin: ExitPin, relays: List<WarrenRelaySummary>): WarrenRelaySummary?

    /**
     * The exit an automatic retry dials once [failedExitPubkeyHex] dropped, or
     * `null` when [pin] leaves no alternative and the same exit is retried: an
     * active alternative inside the pinned scope, in the failed exit's own
     * country first, then anywhere the pin allows, never the failed exit itself.
     * An automatic pin is scoped by [exitCountry] when one is preferred.
     */
    fun failover(
        pin: ExitPin,
        exitCountry: String?,
        relays: List<WarrenRelaySummary>,
        failedExitPubkeyHex: String,
    ): WarrenRelaySummary?
}

/** True when [country] is exactly what is pinned, with no deeper selection. */
fun ExitPin.pinsCountry(country: String): Boolean =
    this is ExitPin.Country && this.country.equals(country, ignoreCase = true)

/** True when [city] in [country] is exactly what is pinned. */
fun ExitPin.pinsCity(country: String, city: String): Boolean =
    this is ExitPin.City &&
        this.country.equals(country, ignoreCase = true) &&
        this.city.equals(city, ignoreCase = true)
