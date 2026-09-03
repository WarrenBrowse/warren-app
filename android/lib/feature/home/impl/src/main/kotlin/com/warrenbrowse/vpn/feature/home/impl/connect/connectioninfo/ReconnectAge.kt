package com.warrenbrowse.vpn.feature.home.impl.connect.connectioninfo

private const val MS_PER_SECOND = 1_000L
private const val SECONDS_PER_MINUTE = 60L
private const val SECONDS_PER_HOUR = 3_600L

/**
 * The age of the last automatic reconnect in the desktop `formatAge` shape ("42s", "12m 7s", "3h
 * 25m"), without the trailing "ago", which the localized `connection_details_age` resource adds
 * around it.
 */
fun formatReconnectAge(ageMs: Long): String {
    val totalSeconds = (ageMs.coerceAtLeast(0L)) / MS_PER_SECOND
    return when {
        totalSeconds < SECONDS_PER_MINUTE -> "${totalSeconds}s"
        totalSeconds < SECONDS_PER_HOUR ->
            "${totalSeconds / SECONDS_PER_MINUTE}m ${totalSeconds % SECONDS_PER_MINUTE}s"
        else ->
            "${totalSeconds / SECONDS_PER_HOUR}h " +
                "${(totalSeconds % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE}m"
    }
}
