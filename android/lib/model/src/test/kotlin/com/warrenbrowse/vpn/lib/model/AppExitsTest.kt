package com.warrenbrowse.vpn.lib.model

import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.junit.jupiter.api.Test

/**
 * Country per app on Android follows the precedence of docs/app-routing.md
 * section 1, the rules the desktop daemon enforces in `mullvad-types`
 * (`effective_app_exits`, `effective_included_apps`) and the desktop GUI shows
 * through `src/shared/app-routing.ts`.
 */
class AppExitsTest {

    private val se = AppExit("se")
    private val deBerlin = AppExit("de", "Berlin")
    private val installed = setOf("org.browser", "org.chat", "org.bank")

    private fun exits(vararg pairs: Pair<String, AppExit>) = mapOf(*pairs)

    @Test
    fun `a country choice is stored lowercase and a blank city means any city`() {
        assertEquals(AppExit("se", null), AppExit.of("SE", " "))
        assertEquals(AppExit("de", "Berlin"), AppExit.of("de", "Berlin"))
        assertNull(AppExit.of("swe", null))
        assertNull(AppExit.of("", null))
    }

    @Test
    fun `no exit is in force while the countries are switched off`() {
        val effective =
            effectiveAppExits(
                SplitTunnelMode.Off,
                excludedApps = emptySet(),
                appExits = exits("org.browser" to se),
                enabled = false,
            )

        assertEquals(emptyMap(), effective)
    }

    @Test
    fun `an excluded app loses its country while exclusion is in force`() {
        val appExits = exits("org.browser" to se, "org.chat" to deBerlin)

        val excluding =
            effectiveAppExits(SplitTunnelMode.Exclude, setOf("org.chat"), appExits, true)
        val notExcluding =
            effectiveAppExits(SplitTunnelMode.IncludeOnly, setOf("org.chat"), appExits, true)

        assertEquals(exits("org.browser" to se), excluding)
        assertEquals(appExits, notExcluding)
    }

    @Test
    fun `include-only tunnels the apps with a country in force as well`() {
        val routing =
            resolveAppRouting(
                SplitTunnelMode.IncludeOnly,
                excludedApps = emptySet(),
                includedApps = setOf("org.bank"),
                isInstalled = { it in installed },
                appsWithCountry = setOf("org.browser", "org.uninstalled"),
            )

        assertEquals(AppRouting.OnlyFor(setOf("org.bank", "org.browser")), routing)
    }

    @Test
    fun `an app with a country alone is enough for include-only to hold a list`() {
        val routing =
            resolveAppRouting(
                SplitTunnelMode.IncludeOnly,
                excludedApps = emptySet(),
                includedApps = emptySet(),
                isInstalled = { it in installed },
                appsWithCountry = setOf("org.chat"),
            )

        assertEquals(AppRouting.OnlyFor(setOf("org.chat")), routing)
    }

    @Test
    fun `a country never pulls an app into another mode's list`() {
        val exclude =
            resolveAppRouting(
                SplitTunnelMode.Exclude,
                excludedApps = setOf("org.chat"),
                includedApps = emptySet(),
                isInstalled = { it in installed },
                appsWithCountry = setOf("org.browser"),
            )

        assertEquals(AppRouting.Bypass(setOf("org.chat")), exclude)
    }

    @Test
    fun `the line under an app says why a country is not in force before any route state`() {
        val statuses =
            listOf(AppRouteStatus(se, AppRouteState.Connected, "192.0.2.7", listOf("org.browser")))

        val paused =
            appRouteLine(
                SplitTunnelMode.Off,
                emptySet(),
                exits("org.browser" to se),
                false,
                statuses,
                "org.browser",
            )
        val bypassed =
            appRouteLine(
                SplitTunnelMode.Exclude,
                setOf("org.browser"),
                exits("org.browser" to se),
                true,
                statuses,
                "org.browser",
            )

        assertEquals(AppRouteLine.Paused, paused)
        assertEquals(AppRouteLine.Bypassed, bypassed)
    }

    @Test
    fun `the line under an app follows its route`() {
        val appExits = exits("org.browser" to se, "org.chat" to deBerlin, "org.bank" to se)
        val statuses =
            listOf(
                AppRouteStatus(se, AppRouteState.Connected, "192.0.2.7", listOf("org.browser")),
                AppRouteStatus(
                    deBerlin,
                    AppRouteState.Unavailable(AppRouteUnavailableReason.WaitingForRoute),
                    null,
                    listOf("org.chat"),
                ),
            )
        fun line(app: String) =
            appRouteLine(SplitTunnelMode.Off, emptySet(), appExits, true, statuses, app)

        assertEquals(AppRouteLine.Connected("192.0.2.7"), line("org.browser"))
        assertEquals(
            AppRouteLine.Unavailable(AppRouteUnavailableReason.WaitingForRoute),
            line("org.chat"),
        )
        assertEquals(AppRouteLine.Waiting, line("org.bank"))
    }

    @Test
    fun `the native status list is read tolerantly`() {
        val json =
            """
            {"routes":[
              {"country":"se","city":null,"state":"connected","public_ip":"192.0.2.7",
               "apps":["org.browser"]},
              {"country":"de","city":"Berlin","state":"unavailable","reason":"waiting_for_route",
               "apps":["org.chat","org.bank"]},
              {"country":"fi","state":"unavailable","reason":"something_new","apps":["org.x"]},
              {"country":"nl","state":"connecting","apps":["org.y"],"extra":1},
              {"state":"connected","apps":["org.z"]}
            ]}
            """

        val parsed = AppRouteStatusParser.parse(json)

        assertEquals(
            listOf(
                AppRouteStatus(se, AppRouteState.Connected, "192.0.2.7", listOf("org.browser")),
                AppRouteStatus(
                    deBerlin,
                    AppRouteState.Unavailable(AppRouteUnavailableReason.WaitingForRoute),
                    null,
                    listOf("org.chat", "org.bank"),
                ),
                AppRouteStatus(
                    AppExit("fi"),
                    AppRouteState.Unavailable(null),
                    null,
                    listOf("org.x"),
                ),
                AppRouteStatus(AppExit("nl"), AppRouteState.Connecting, null, listOf("org.y")),
            ),
            parsed,
        )
        assertEquals(emptyList(), AppRouteStatusParser.parse("not json"))
    }

    @Test
    fun `a route on a network that reaches no entry server reads under its own reason`() {
        // The native side names it `no_dialable_network` (topic 210); read as
        // an unknown reason it would fall back to the generic line.
        val json =
            """
            {"routes":[{"country":"se","state":"unavailable","reason":"no_dialable_network",
              "apps":["org.browser"]}]}
            """

        assertEquals(
            listOf(
                AppRouteStatus(
                    se,
                    AppRouteState.Unavailable(AppRouteUnavailableReason.NoDialableNetwork),
                    null,
                    listOf("org.browser"),
                )
            ),
            AppRouteStatusParser.parse(json),
        )
    }

    @Test
    fun `an app counted with a country is one whose country is in force`() {
        val appExits = exits("org.browser" to se, "org.chat" to deBerlin)

        assertEquals(
            1,
            effectiveAppExits(SplitTunnelMode.Exclude, setOf("org.chat"), appExits, true).size,
        )
    }
}
