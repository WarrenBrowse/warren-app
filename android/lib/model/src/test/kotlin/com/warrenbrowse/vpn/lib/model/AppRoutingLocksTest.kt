package com.warrenbrowse.vpn.lib.model

import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.junit.jupiter.api.Nested
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertThrows

/**
 * Apps locked to the VPN ("Never without the VPN", docs/app-routing.md section 8). The same cases
 * as the desktop `app-routing-locks.spec.ts`.
 */
class AppRoutingLocksTest {

    private val firefox = "org.mozilla.firefox"
    private val slack = "com.Slack"
    private val steam = "com.valvesoftware.android.steam.community"
    private val signal = "org.thoughtcrime.securesms"
    private val apps = listOf(firefox, slack, steam, signal)

    private val nl = AppExit("nl")

    private fun settings(
        splitMode: SplitTunnelMode = SplitTunnelMode.Off,
        excludedApps: Set<String> = emptySet(),
        includedApps: Set<String> = emptySet(),
        appExits: Map<String, AppExit> = emptyMap(),
        lockedApps: Set<String> = emptySet(),
    ) = AppRoutingSettings(splitMode, excludedApps, includedApps, true, appExits, lockedApps)

    private fun statesAlong(
        routing: AppRoutingSettings,
        ops: List<RoutingOp>,
    ): List<AppRoutingSettings> = ops.runningFold(routing) { state, op -> state.apply(listOf(op)) }

    // A lock change never moves the app or any other: the lock decides only what happens while the
    // VPN does not carry the app.
    private fun expectCleanLock(
        routing: AppRoutingSettings,
        app: String,
        locked: Boolean,
    ): AppRoutingSettings {
        val states = statesAlong(routing, routing.planAppLock(app, locked))
        for (state in states) {
            for (any in apps) assertEquals(routing.routeOf(any), state.routeOf(any), "$any moved")
        }
        assertEquals(locked, app in states.last().lockedApps)
        return states.last()
    }

    @Nested
    inner class InTheList {
        @Test
        fun `a locked app is a rule even when it takes the default route`() {
            assertEquals(
                listOf(AppRule(firefox, AppRoute.Vpn, locked = true)),
                settings(lockedApps = setOf(firefox)).rules(),
            )
        }

        @Test
        fun `a locked app is never outside the VPN, whatever list it is also in`() {
            val routing =
                settings(
                    SplitTunnelMode.Exclude,
                    excludedApps = setOf(firefox, steam),
                    lockedApps = setOf(firefox),
                )

            assertEquals(AppRoute.Vpn, routing.routeOf(firefox))
            assertEquals(
                AppRouting.Bypass(setOf(steam)),
                routing.tunnelRouting { true },
            )
        }

        @Test
        fun `a locked app is on the VPN when the other apps go outside it`() {
            val routing = settings(SplitTunnelMode.IncludeOnly, lockedApps = setOf(signal))

            assertEquals(AppRoute.Vpn, routing.routeOf(signal))
            assertEquals(AppRouting.OnlyFor(setOf(signal)), routing.tunnelRouting { true })
        }

        @Test
        fun `a locked app keeps its country even when listed as excluded`() {
            val routing =
                settings(
                    SplitTunnelMode.Exclude,
                    excludedApps = setOf(firefox),
                    appExits = mapOf(firefox to nl),
                    lockedApps = setOf(firefox),
                )

            assertEquals(AppRoute.Country(nl), routing.routeOf(firefox))
            assertEquals(mapOf(firefox to nl), routing.effectiveAppExits)
        }
    }

    @Nested
    inner class PlanAppLock {
        @Test
        fun `locks an app on the VPN in one write`() {
            assertEquals(listOf(RoutingOp.Lock(firefox)), settings().planAppLock(firefox, true))
            expectCleanLock(settings(), firefox, true)
        }

        @Test
        fun `locks an app with a country without moving it`() {
            expectCleanLock(settings(appExits = mapOf(firefox to nl)), firefox, true)
        }

        @Test
        fun `refuses to lock an app outside the VPN`() {
            assertThrows<IllegalArgumentException> {
                settings(SplitTunnelMode.Exclude, excludedApps = setOf(steam))
                    .planAppLock(steam, true)
            }
            assertThrows<IllegalArgumentException> {
                settings(SplitTunnelMode.IncludeOnly).planAppLock(steam, true)
            }
        }

        @Test
        fun `keeps on the VPN an app only the lock put there`() {
            val after =
                expectCleanLock(
                    settings(SplitTunnelMode.IncludeOnly, lockedApps = setOf(signal)),
                    signal,
                    false,
                )

            assertEquals(setOf(signal), after.includedApps)
        }

        @Test
        fun `drops an exclusion the lock was overriding`() {
            val after =
                expectCleanLock(
                    settings(
                        SplitTunnelMode.Exclude,
                        excludedApps = setOf(firefox),
                        lockedApps = setOf(firefox),
                    ),
                    firefox,
                    false,
                )

            assertEquals(emptySet(), after.excludedApps)
            assertEquals(SplitTunnelMode.Off, after.splitMode)
        }

        @Test
        fun `does nothing when the app is already as asked`() {
            assertEquals(emptyList(), settings().planAppLock(firefox, false))
            assertEquals(
                emptyList(),
                settings(lockedApps = setOf(firefox)).planAppLock(firefox, true),
            )
        }
    }

    @Nested
    inner class RouteChanges {
        @Test
        fun `lifts the lock first when the app goes outside the VPN`() {
            val routing = settings(appExits = mapOf(firefox to nl), lockedApps = setOf(firefox))
            val ops = routing.planAppRoute(firefox, AppRoute.Direct)

            assertEquals(RoutingOp.Unlock(firefox), ops.first())
            for (state in statesAlong(routing, ops)) {
                assertTrue(state.routeOf(firefox) in listOf(AppRoute.Country(nl), AppRoute.Direct))
            }
            assertEquals(AppRoute.Direct, routing.apply(ops).routeOf(firefox))
        }

        @Test
        fun `keeps the lock when the app moves to a country`() {
            val routing = settings(lockedApps = setOf(firefox))

            val after = routing.apply(routing.planAppRoute(firefox, AppRoute.Country(nl)))

            assertTrue(firefox in after.lockedApps)
        }

        @Test
        fun `keeps every locked app on the VPN at each step toward direct as the default`() {
            val routing = settings(lockedApps = setOf(firefox))
            val ops = routing.planDefaultRoute(DefaultRoute.Direct)

            for (state in statesAlong(routing, ops)) {
                assertEquals(AppRoute.Vpn, state.routeOf(firefox))
            }
            assertEquals(setOf(firefox), routing.apply(ops).lockedApps)
        }
    }

    @Nested
    inner class Guard {
        @Test
        fun `captures exactly the locked apps on the device`() {
            val routing = settings(lockedApps = setOf(firefox, slack))

            assertEquals(
                AppRouting.OnlyFor(setOf(firefox)),
                routing.lockGuardRouting { it == firefox },
            )
        }

        @Test
        fun `captures nothing when no locked app is on the device`() {
            // An empty allow list would capture every app.
            assertNull(settings(lockedApps = setOf(firefox)).lockGuardRouting { false })
            assertNull(settings().lockGuardRouting { true })
        }

        @Test
        fun `locking drops the exclusion, unlocking does not bring it back`() {
            val routing = settings(SplitTunnelMode.Exclude, excludedApps = setOf(firefox, steam))

            val after =
                routing.apply(listOf(RoutingOp.Lock(firefox), RoutingOp.Unlock(firefox)))

            assertEquals(setOf(steam), after.excludedApps)
            assertFalse(firefox in after.lockedApps)
        }
    }
}
