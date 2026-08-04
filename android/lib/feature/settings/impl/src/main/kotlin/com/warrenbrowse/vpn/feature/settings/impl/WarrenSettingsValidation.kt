package com.warrenbrowse.vpn.feature.settings.impl

import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository

/**
 * Pure validation and derivation behind the Warren settings fields, kept out of
 * the Composables so the commit rules are exercised on the JVM.
 *
 * Every numeric field here follows the same desktop contract: the typed text
 * lives in a local buffer, the field states its valid range, and a value is
 * persisted only once it is inside that range. A blank field is always valid
 * and means the documented "no override" value, never zero-by-accident.
 */
internal object SettingsRanges {
    /** TUN MTU bounds, enforced by the repository at write time as well. */
    const val MTU_MIN = WarrenLocalSettingsRepository.MTU_MIN
    const val MTU_MAX = WarrenLocalSettingsRepository.MTU_MAX

    /** Bandwidth ceiling in Mbps. Blank (no limit) is handled separately. */
    const val MAX_RATE_MBPS_MIN = 1
    const val MAX_RATE_MBPS_MAX = 10_000

    /**
     * Preferred external port. The range must match the exit allocator
     * (warren-config NATPMP_EXTERNAL_PORT_MIN / NATPMP_EXTERNAL_PORT_MAX): a
     * port outside it can never be honoured, so it is refused client-side
     * rather than surfacing later as an opaque "Status: failed".
     */
    const val NATPMP_PORT_MIN = 49_152
    const val NATPMP_PORT_MAX = 65_535
}

/**
 * True when [input] is blank (the documented "no override" value, always
 * accepted) or parses into [range].
 */
private fun inputIsValid(input: String, range: IntRange): Boolean =
    input.isBlank() || (input.toIntOrNull()?.let { it in range } == true)

/** True when [input] is blank (the default) or an in-range MTU. */
internal fun mtuInputIsValid(input: String): Boolean =
    inputIsValid(input, SettingsRanges.MTU_MIN..SettingsRanges.MTU_MAX)

/** True when [input] is blank (unlimited) or an in-range Mbps figure. */
internal fun maxRateInputIsValid(input: String): Boolean =
    inputIsValid(input, SettingsRanges.MAX_RATE_MBPS_MIN..SettingsRanges.MAX_RATE_MBPS_MAX)

/** True when [input] is blank (the exit chooses) or a port the exit can honour. */
internal fun natPmpPortInputIsValid(input: String): Boolean =
    inputIsValid(input, SettingsRanges.NATPMP_PORT_MIN..SettingsRanges.NATPMP_PORT_MAX)

/** The port to persist for [input]: 0 (let the exit choose) when blank. */
internal fun natPmpPortFromInput(input: String): Int = input.toIntOrNull() ?: 0

/**
 * True when [value] is a numeric IP literal. Written in Kotlin rather than
 * delegating to `android.net.InetAddresses` so the rule is testable on the JVM,
 * where the framework stub throws.
 */
internal fun isNumericIpAddress(value: String): Boolean =
    isIpv4Literal(value) || isIpv6Literal(value)

private const val IPV4_OCTETS = 4
private const val IPV4_OCTET_MAX_DIGITS = 3
private const val IPV4_OCTET_MAX = 255
private const val IPV6_GROUPS = 8
private const val IPV6_GROUP_MAX_DIGITS = 4

private fun isIpv4Literal(value: String): Boolean {
    val parts = value.split('.')
    return parts.size == IPV4_OCTETS && parts.all(::isIpv4Octet)
}

private fun isIpv4Octet(part: String): Boolean =
    part.isNotEmpty() &&
        part.length <= IPV4_OCTET_MAX_DIGITS &&
        part.all(Char::isDigit) &&
        // A leading zero is ambiguous (some resolvers read it as octal), so it
        // is refused rather than silently reinterpreted.
        (part.length == 1 || part[0] != '0') &&
        part.toInt() <= IPV4_OCTET_MAX

private fun isIpv6Literal(value: String): Boolean {
    val doubleColons = Regex("::").findAll(value).count()
    // "::" itself is the all-zero address; splitting it yields only blanks, so
    // a compressed literal is counted against the group budget, not the exact
    // group count.
    val compressed = doubleColons == 1
    val groups = value.split(':').filter { it.isNotEmpty() }
    val shapeIsValid = value.contains(':') &&
        doubleColons <= 1 &&
        (compressed || !(value.startsWith(':') || value.endsWith(':'))) &&
        (if (compressed) groups.size < IPV6_GROUPS else groups.size == IPV6_GROUPS)
    return shapeIsValid && groups.all(::isIpv6Group)
}

private fun isIpv6Group(group: String): Boolean =
    group.length <= IPV6_GROUP_MAX_DIGITS &&
        group.all { it.isDigit() || it.lowercaseChar() in 'a'..'f' }

/** The resolver list to persist for a raw comma/newline separated [input]. */
internal fun customDnsServersFromInput(input: String): List<String> =
    input.split(',', '\n').map { it.trim() }.filter { it.isNotEmpty() }

/**
 * True when every token in [input] parses as a numeric IP and no address is
 * repeated. An empty list is valid so the field can be cleared.
 */
internal fun customDnsInputIsValid(input: String): Boolean {
    val tokens = customDnsServersFromInput(input)
    return tokens.all(::isNumericIpAddress) && tokens.distinct().size == tokens.size
}

/**
 * Whether the port-forwarding controls may be used right now, derived from the
 * NAT-PMP status snapshot and how long ago it arrived.
 *
 * The exit rate-limits port allocations per source over a sliding window, so a
 * client that keeps editing while a window is open walks itself into a ban.
 * [blocked] disables the controls; [lastChance] warns that exactly one slot is
 * left. Both expire purely on the clock, so a stale snapshot (the exit only
 * pushes on a mapping event) can never strand the controls.
 */
internal data class NatPmpPortBlock(
    val blocked: Boolean,
    val lastChance: Boolean,
    val remainingSecs: Int,
)

private val NO_PORT_BLOCK =
    NatPmpPortBlock(blocked = false, lastChance = false, remainingSecs = 0)

/** The open rate-limit window this status describes: its length, and whether it blocks. */
private fun natPmpWindow(json: String): Pair<Int, Boolean>? =
    when (jsonField(json, "state")) {
        "rate_limited" -> (jsonField(json, "retry_after_secs")?.toIntOrNull() ?: 0) to true
        // `attempts_remaining` is absent on a pre-trailer exit: no budget to
        // reason about, so nothing is blocked or warned.
        "mapped" -> {
            val window = jsonField(json, "window_reset_secs")?.toIntOrNull() ?: 0
            when (jsonField(json, "attempts_remaining")?.toIntOrNull()) {
                0 -> window to true
                1 -> window to false
                else -> null
            }
        }
        else -> null
    }

internal fun natPmpPortBlock(json: String, elapsedSecs: Long): NatPmpPortBlock {
    val window = natPmpWindow(json)
    val remaining = ((window?.first ?: 0) - elapsedSecs).coerceAtLeast(0L).toInt()
    // A zero-length or elapsed window carries no live information: past it the
    // exit has recovered at least one slot, so keeping the controls disabled
    // (or the warning up) would strand the user on a stale snapshot.
    return if (window == null || remaining == 0) {
        NO_PORT_BLOCK
    } else {
        NatPmpPortBlock(
            blocked = window.second,
            lastChance = !window.second,
            remainingSecs = remaining,
        )
    }
}

private const val SECONDS_PER_MINUTE = 60
private const val COUNTDOWN_FIELD_WIDTH = 2

/** Formats a whole-second count as `mm:ss` for countdown display. */
internal fun formatCountdown(totalSecs: Int): String {
    val minutes = (totalSecs / SECONDS_PER_MINUTE).toString().padStart(COUNTDOWN_FIELD_WIDTH, '0')
    val seconds = (totalSecs % SECONDS_PER_MINUTE).toString().padStart(COUNTDOWN_FIELD_WIDTH, '0')
    return "$minutes:$seconds"
}
