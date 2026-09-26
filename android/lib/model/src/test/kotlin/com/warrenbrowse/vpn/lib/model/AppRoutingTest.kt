package com.warrenbrowse.vpn.lib.model

import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

/**
 * The split mode and the two app lists resolve to what the TUN captures
 * (docs/app-routing.md, sections 1 and 3). The include-only guard is the part
 * that matters for privacy: Android answers an allow list that ends up empty by
 * capturing nothing of it, so a list with no installed app must fall back to a
 * full tunnel rather than reach the platform.
 */
class AppRoutingTest {

    private val installed = setOf("org.browser", "org.chat", "org.bank")

    private fun resolve(
        mode: SplitTunnelMode,
        excluded: Set<String> = emptySet(),
        included: Set<String> = emptySet(),
    ) = resolveAppRouting(mode, excluded, included) { it in installed }

    @Test
    fun `off tunnels every app whatever the lists hold`() {
        assertEquals(
            AppRouting.AllApps,
            resolve(SplitTunnelMode.Off, excluded = setOf("org.chat"), included = setOf("org.bank")),
        )
    }

    @Test
    fun `exclude bypasses the excluded apps and ignores the included list`() {
        assertEquals(
            AppRouting.Bypass(setOf("org.chat")),
            resolve(
                SplitTunnelMode.Exclude,
                excluded = setOf("org.chat"),
                included = setOf("org.bank"),
            ),
        )
    }

    @Test
    fun `exclude with an empty list tunnels every app`() {
        assertEquals(AppRouting.AllApps, resolve(SplitTunnelMode.Exclude))
    }

    @Test
    fun `include-only tunnels only the installed included apps`() {
        assertEquals(
            AppRouting.OnlyFor(setOf("org.browser", "org.bank")),
            resolve(
                SplitTunnelMode.IncludeOnly,
                excluded = setOf("org.chat"),
                included = setOf("org.browser", "org.bank", "org.uninstalled"),
            ),
        )
    }

    @Test
    fun `include-only with an empty list falls back to a full tunnel`() {
        assertEquals(AppRouting.AllApps, resolve(SplitTunnelMode.IncludeOnly))
    }

    @Test
    fun `include-only whose apps are all uninstalled falls back to a full tunnel`() {
        assertEquals(
            AppRouting.AllApps,
            resolve(SplitTunnelMode.IncludeOnly, included = setOf("org.gone", "org.removed")),
        )
    }
}
