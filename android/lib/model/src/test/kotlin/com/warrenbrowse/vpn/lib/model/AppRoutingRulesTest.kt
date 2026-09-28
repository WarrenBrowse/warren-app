package com.warrenbrowse.vpn.lib.model

import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue
import org.junit.jupiter.api.Nested
import org.junit.jupiter.api.Test

/**
 * The single list of App routing and its translation to the settings model (docs/app-routing.md
 * section 3.5). The same cases as the desktop `app-routing-rules.spec.ts`, since both clients show
 * one route per app over the same split mode, lists and countries.
 */
class AppRoutingRulesTest {

    private val firefox = "org.mozilla.firefox"
    private val slack = "com.Slack"
    private val steam = "com.valvesoftware.android.steam.community"
    private val signal = "org.thoughtcrime.securesms"
    private val apps = listOf(firefox, slack, steam, signal)

    private val ro = AppExit("ro")
    private val nl = AppExit("nl")
    private val deBer = AppExit("de", "Berlin")

    private fun settings(
        splitMode: SplitTunnelMode = SplitTunnelMode.Off,
        excludedApps: Set<String> = emptySet(),
        includedApps: Set<String> = emptySet(),
        appExitsEnabled: Boolean = true,
        appExits: Map<String, AppExit> = emptyMap(),
    ) = AppRoutingSettings(splitMode, excludedApps, includedApps, appExitsEnabled, appExits)

    // Every state the settings pass through while the operations apply one by one, the starting
    // state first.
    private fun statesAlong(
        routing: AppRoutingSettings,
        ops: List<RoutingOp>,
    ): List<AppRoutingSettings> = ops.runningFold(routing) { state, op -> state.apply(listOf(op)) }

    // The whole contract of a change to one app: it ends on the route asked for, never passes
    // through a route that is neither the old nor the new one (each operation is a separate
    // settings write the tunnel follows), and no other app moves at any step.
    private fun expectCleanChange(
        routing: AppRoutingSettings,
        app: String,
        next: AppRoute,
    ): AppRoutingSettings {
        val before = routing.routeOf(app)
        val states = statesAlong(routing, routing.planAppRoute(app, next))
        for (state in states) {
            assertTrue(state.routeOf(app) in listOf(before, next), "$app took ${state.routeOf(app)}")
            for (other in apps.filter { it != app }) {
                assertEquals(routing.routeOf(other), state.routeOf(other), "$other moved")
            }
        }
        val after = states.last()
        assertEquals(next, after.routeOf(app))
        return after
    }

    @Test
    fun `the default is the VPN unless VPN only for is on`() {
        assertEquals(DefaultRoute.Vpn, settings().defaultRoute)
        assertEquals(DefaultRoute.Vpn, settings(SplitTunnelMode.Exclude).defaultRoute)
        assertEquals(DefaultRoute.Direct, settings(SplitTunnelMode.IncludeOnly).defaultRoute)
    }

    @Nested
    inner class Rules {
        @Test
        fun `a fresh install shows no rule`() {
            assertEquals(emptyList(), settings().rules())
        }

        @Test
        fun `shows excluded apps outside the VPN and apps with a country, the VPN being the default`() {
            val routing =
                settings(
                    SplitTunnelMode.Exclude,
                    excludedApps = setOf(steam),
                    appExits = mapOf(firefox to nl),
                )
            assertEquals(
                listOf(AppRule(steam, AppRoute.Direct), AppRule(firefox, AppRoute.Country(nl))),
                routing.rules(),
            )
        }

        @Test
        fun `shows the included apps on the VPN and apps with a country, direct being the default`() {
            val routing =
                settings(
                    SplitTunnelMode.IncludeOnly,
                    includedApps = setOf(signal),
                    appExits = mapOf(firefox to ro),
                )
            assertEquals(
                listOf(AppRule(signal, AppRoute.Vpn), AppRule(firefox, AppRoute.Country(ro))),
                routing.rules(),
            )
        }

        @Test
        fun `shows one rule per app, the one the tunnel applies`() {
            val excludedWithCountry =
                settings(
                    SplitTunnelMode.Exclude,
                    excludedApps = setOf(steam),
                    appExits = mapOf(steam to nl),
                )
            assertEquals(listOf(AppRule(steam, AppRoute.Direct)), excludedWithCountry.rules())

            val includedWithCountry =
                settings(
                    SplitTunnelMode.IncludeOnly,
                    includedApps = setOf(steam),
                    appExits = mapOf(steam to nl),
                )
            assertEquals(listOf(AppRule(steam, AppRoute.Country(nl))), includedWithCountry.rules())
        }

        @Test
        fun `hides what is saved but not in force`() {
            val routing =
                settings(
                    SplitTunnelMode.Off,
                    excludedApps = setOf(steam),
                    includedApps = setOf(signal),
                    appExitsEnabled = false,
                    appExits = mapOf(firefox to nl),
                )
            assertEquals(emptyList(), routing.rules())
        }

        @Test
        fun `tells package names apart by case, as Android does`() {
            val routing = settings(SplitTunnelMode.Exclude, excludedApps = setOf("com.slack"))
            assertEquals(AppRoute.Vpn, routing.routeOf(slack))
        }
    }

    @Nested
    inner class WithTheVpnAsTheDefault {
        @Test
        fun `bypass turns on for the first app sent outside, dropping a list saved while it was off`() {
            val routing = settings(SplitTunnelMode.Off, excludedApps = setOf(slack))
            val after = expectCleanChange(routing, steam, AppRoute.Direct)
            assertEquals(SplitTunnelMode.Exclude, after.splitMode)
            assertEquals(setOf(steam), after.excludedApps)
        }

        @Test
        fun `adds to bypass without touching the mode when it is already on`() {
            val routing = settings(SplitTunnelMode.Exclude, excludedApps = setOf(slack))
            assertEquals(
                listOf(RoutingOp.AddExcluded(steam)),
                routing.planAppRoute(steam, AppRoute.Direct),
            )
            expectCleanChange(routing, steam, AppRoute.Direct)
        }

        @Test
        fun `bypass turns off with the last app brought back to the VPN`() {
            val routing = settings(SplitTunnelMode.Exclude, excludedApps = setOf(steam))
            val after = expectCleanChange(routing, steam, AppRoute.Vpn)
            assertEquals(SplitTunnelMode.Off, after.splitMode)
            assertEquals(emptySet(), after.excludedApps)
        }

        @Test
        fun `bypass stays on while other apps still bypass`() {
            val routing = settings(SplitTunnelMode.Exclude, excludedApps = setOf(steam, slack))
            val after = expectCleanChange(routing, steam, AppRoute.Vpn)
            assertEquals(SplitTunnelMode.Exclude, after.splitMode)
            assertEquals(setOf(slack), after.excludedApps)
        }

        @Test
        fun `a country needs no switch, and drops the countries saved while they were off`() {
            val routing = settings(appExitsEnabled = false, appExits = mapOf(slack to nl))
            val after = expectCleanChange(routing, firefox, AppRoute.Country(ro))
            assertTrue(after.appExitsEnabled)
            assertEquals(mapOf(firefox to ro), after.appExits)
        }

        @Test
        fun `an app moves from one country to another in one write`() {
            val routing = settings(appExits = mapOf(firefox to nl))
            assertEquals(
                listOf(RoutingOp.SetExit(firefox, deBer)),
                routing.planAppRoute(firefox, AppRoute.Country(deBer)),
            )
            expectCleanChange(routing, firefox, AppRoute.Country(deBer))
        }

        @Test
        fun `a bypassing app moves to a country, and back outside the VPN`() {
            val bypassing = settings(SplitTunnelMode.Exclude, excludedApps = setOf(steam))
            val withCountry = expectCleanChange(bypassing, steam, AppRoute.Country(nl))
            assertEquals(SplitTunnelMode.Off, withCountry.splitMode)
            val back = expectCleanChange(withCountry, steam, AppRoute.Direct)
            assertEquals(emptyMap(), back.appExits)
        }

        @Test
        fun `an app saved both as bypassing and with a country loses both on its way to the VPN`() {
            val routing =
                settings(
                    SplitTunnelMode.Exclude,
                    excludedApps = setOf(steam),
                    appExits = mapOf(steam to nl),
                )
            val after = expectCleanChange(routing, steam, AppRoute.Vpn)
            assertEquals(emptyMap(), after.appExits)
            assertEquals(emptySet(), after.excludedApps)
        }

        @Test
        fun `nothing is written when the app already takes that route`() {
            assertEquals(emptyList(), settings().planAppRoute(firefox, AppRoute.Vpn))
            val routing = settings(appExits = mapOf(firefox to nl))
            assertEquals(
                emptyList(),
                routing.planAppRoute(firefox, AppRoute.Country(AppExit("nl"))),
            )
        }
    }

    @Nested
    inner class WithDirectAsTheDefault {
        private val includeOnly = SplitTunnelMode.IncludeOnly

        @Test
        fun `an app is put on the VPN`() {
            val after = expectCleanChange(settings(includeOnly), signal, AppRoute.Vpn)
            assertEquals(setOf(signal), after.includedApps)
        }

        @Test
        fun `an app with a country never leaves the VPN on its way back to the main country`() {
            val routing = settings(includeOnly, appExits = mapOf(firefox to ro))
            val after = expectCleanChange(routing, firefox, AppRoute.Vpn)
            assertEquals(emptyMap(), after.appExits)
            assertEquals(setOf(firefox), after.includedApps)
        }

        @Test
        fun `an included app never leaves the VPN on its way to a country`() {
            val routing = settings(includeOnly, includedApps = setOf(signal), appExitsEnabled = false)
            val after = expectCleanChange(routing, signal, AppRoute.Country(nl))
            assertEquals(emptySet(), after.includedApps)
            assertTrue(after.appExitsEnabled)
        }

        @Test
        fun `an app goes back outside the VPN by losing its rule`() {
            val routing =
                settings(includeOnly, includedApps = setOf(firefox), appExits = mapOf(firefox to ro))
            val after = expectCleanChange(routing, firefox, AppRoute.Direct)
            assertEquals(emptySet(), after.includedApps)
            assertEquals(emptyMap(), after.appExits)
        }

        @Test
        fun `the mode stays when the last rule goes, since direct is what the user chose`() {
            val routing = settings(includeOnly, includedApps = setOf(signal))
            val after = expectCleanChange(routing, signal, AppRoute.Direct)
            assertEquals(SplitTunnelMode.IncludeOnly, after.splitMode)
        }
    }

    @Nested
    inner class DefaultChange {
        @Test
        fun `toward direct, VPN only for turns on, the countries stay and both lists go`() {
            val routing =
                settings(
                    SplitTunnelMode.Exclude,
                    excludedApps = setOf(steam),
                    includedApps = setOf(slack),
                    appExits = mapOf(firefox to nl),
                )
            val after = routing.apply(routing.planDefaultRoute(DefaultRoute.Direct))
            assertEquals(SplitTunnelMode.IncludeOnly, after.splitMode)
            assertEquals(emptySet(), after.excludedApps)
            // A list saved earlier would come back as rules nobody just chose.
            assertEquals(emptySet(), after.includedApps)
            assertEquals(listOf(AppRule(firefox, AppRoute.Country(nl))), after.rules())
        }

        @Test
        fun `toward the VPN, VPN only for turns off first, so no app is outside on the way`() {
            val routing =
                settings(
                    SplitTunnelMode.IncludeOnly,
                    includedApps = setOf(signal),
                    excludedApps = setOf(steam),
                    appExits = mapOf(firefox to ro),
                )
            val ops = routing.planDefaultRoute(DefaultRoute.Vpn)
            assertEquals(RoutingOp.SetSplitMode(SplitTunnelMode.Off), ops.first())
            val after = routing.apply(ops)
            assertEquals(emptySet(), after.includedApps)
            assertEquals(emptySet(), after.excludedApps)
            assertEquals(listOf(AppRule(firefox, AppRoute.Country(ro))), after.rules())
            for (state in statesAlong(routing, ops).drop(1)) {
                for (app in apps) assertNotEquals(AppRoute.Direct, state.routeOf(app))
            }
        }

        @Test
        fun `nothing is written when the default is already the one asked for`() {
            assertEquals(emptyList(), settings().planDefaultRoute(DefaultRoute.Vpn))
            assertEquals(
                emptyList(),
                settings(SplitTunnelMode.IncludeOnly).planDefaultRoute(DefaultRoute.Direct),
            )
        }
    }

    /**
     * Android's own part: an include-only list with no app on the device runs as a full tunnel
     * (section 3.4), so a change that turns it into a list moves every other app out of the VPN
     * at once, and the screen asks first.
     */
    @Nested
    inner class Narrowing {
        private val installed = setOf(firefox, slack, signal)

        private fun AppRoutingSettings.narrows(ops: List<RoutingOp>) =
            changeNarrowsTunnel(this, apply(ops)) { it in installed }

        @Test
        fun `the first rule with direct as the default narrows the full tunnel to that app`() {
            val routing = settings(SplitTunnelMode.IncludeOnly)
            assertTrue(routing.narrows(routing.planAppRoute(signal, AppRoute.Vpn)))
            assertTrue(routing.narrows(routing.planAppRoute(firefox, AppRoute.Country(nl))))
        }

        @Test
        fun `a rule for an app that is not on the device leaves the full tunnel alone`() {
            val routing = settings(SplitTunnelMode.IncludeOnly)
            assertFalse(routing.narrows(routing.planAppRoute(steam, AppRoute.Vpn)))
        }

        @Test
        fun `a second rule only widens the list`() {
            val routing = settings(SplitTunnelMode.IncludeOnly, includedApps = setOf(signal))
            assertFalse(routing.narrows(routing.planAppRoute(firefox, AppRoute.Vpn)))
        }

        @Test
        fun `direct as the default narrows the tunnel to the apps with a country`() {
            val routing =
                settings(
                    SplitTunnelMode.Exclude,
                    excludedApps = setOf(slack),
                    appExits = mapOf(firefox to nl),
                )
            assertTrue(routing.narrows(routing.planDefaultRoute(DefaultRoute.Direct)))
        }

        @Test
        fun `direct as the default with no country keeps the full tunnel`() {
            val routing = settings(SplitTunnelMode.Exclude, excludedApps = setOf(slack))
            assertFalse(routing.narrows(routing.planDefaultRoute(DefaultRoute.Direct)))
        }

        @Test
        fun `nothing narrows with the VPN as the default`() {
            val routing = settings()
            assertFalse(routing.narrows(routing.planAppRoute(slack, AppRoute.Direct)))
            assertFalse(routing.narrows(routing.planAppRoute(slack, AppRoute.Country(nl))))
        }
    }
}
