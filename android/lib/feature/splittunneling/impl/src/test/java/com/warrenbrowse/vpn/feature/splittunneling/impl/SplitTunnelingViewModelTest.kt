package com.warrenbrowse.vpn.feature.splittunneling.impl

import androidx.lifecycle.viewModelScope
import app.cash.turbine.ReceiveTurbine
import app.cash.turbine.test
import io.mockk.coVerify
import io.mockk.every
import io.mockk.mockk
import io.mockk.slot
import io.mockk.unmockkAll
import java.util.concurrent.TimeUnit
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.runTest
import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.AppData
import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.ApplicationsProvider
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.AppExit
import com.warrenbrowse.vpn.lib.model.AppRoute
import com.warrenbrowse.vpn.lib.model.AppRouteLine
import com.warrenbrowse.vpn.lib.model.AppRouteState
import com.warrenbrowse.vpn.lib.model.AppRouteStatus
import com.warrenbrowse.vpn.lib.model.AppRouting
import com.warrenbrowse.vpn.lib.model.AppRoutingSettings
import com.warrenbrowse.vpn.lib.model.DefaultRoute
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.model.RoutingOp
import com.warrenbrowse.vpn.lib.model.SplitTunnelMode
import com.warrenbrowse.vpn.lib.model.changeNarrowsTunnel
import com.warrenbrowse.vpn.lib.repository.SplitTunnelingRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.lib.repository.WarrenAppRoutesStatusProvider
import com.warrenbrowse.vpn.lib.repository.WarrenRelayProvider
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.Timeout
import org.junit.jupiter.api.extension.ExtendWith

/**
 * App routing's single list over the settings model. The repository is backed by the settings it
 * would write, so each test observes where the apps end up rather than which call was made.
 */
@ExperimentalCoroutinesApi
@ExtendWith(TestCoroutineRule::class)
@Timeout(3000L, unit = TimeUnit.MILLISECONDS)
class SplitTunnelingViewModelTest {

    private val applicationsProvider = mockk<ApplicationsProvider>()
    private val repository = mockk<SplitTunnelingRepository>(relaxed = true)
    private val preferences = mockk<UserPreferencesRepository>(relaxed = true)
    private val relayProvider = mockk<WarrenRelayProvider>(relaxed = true)
    private lateinit var testSubject: SplitTunnelingViewModel

    private val splitMode = MutableStateFlow(SplitTunnelMode.Off)
    private val excludedApps = MutableStateFlow<Set<PackageName>>(emptySet())
    private val includedApps = MutableStateFlow<Set<PackageName>>(emptySet())
    private val appExits = MutableStateFlow<Map<String, AppExit>>(emptyMap())
    private val appExitsEnabled = MutableStateFlow(false)
    private val lockedApps = MutableStateFlow<Set<String>>(emptySet())
    private val tunnelConnected = MutableStateFlow(true)
    private val vpnOnlyForCount = MutableStateFlow<Int?>(null)
    private val showSystemApps = MutableStateFlow(false)
    private val appRoutes = MutableStateFlow<List<AppRouteStatus>>(emptyList())
    private val catalogue = MutableStateFlow<List<WarrenRelaySummary>>(emptyList())
    private val appRoutesProvider =
        object : WarrenAppRoutesStatusProvider {
            override val appRoutes = this@SplitTunnelingViewModelTest.appRoutes
        }
    private val writes = mutableListOf<RoutingOp>()

    private val bank = AppData(PackageName("org.bank"), 0, "Bank")
    private val chat = AppData(PackageName("org.chat"), 0, "Chat")
    private val maps = AppData(PackageName("org.maps"), 0, "Maps")
    private val clock = AppData(PackageName("android.clock"), 0, "Clock", isSystemApp = true)

    private fun settings() =
        AppRoutingSettings(
            splitMode.value,
            excludedApps.value.mapTo(LinkedHashSet()) { it.value },
            includedApps.value.mapTo(LinkedHashSet()) { it.value },
            appExitsEnabled.value,
            appExits.value,
            lockedApps.value,
        )

    private fun store(next: AppRoutingSettings) {
        splitMode.value = next.splitMode
        excludedApps.value = next.excludedApps.mapTo(LinkedHashSet(), ::PackageName)
        includedApps.value = next.includedApps.mapTo(LinkedHashSet(), ::PackageName)
        appExitsEnabled.value = next.appExitsEnabled
        appExits.value = next.appExits
        lockedApps.value = next.lockedApps
        vpnOnlyForCount.value =
            (next.tunnelRouting(::installed) as? AppRouting.OnlyFor)?.packages?.size
    }

    private fun installed(app: String) = app in setOf(bank, chat, maps, clock).map { it.packageName.value }

    @BeforeEach
    fun setup() {
        every { repository.splitMode } returns splitMode
        every { repository.excludedApps } returns excludedApps
        every { repository.includedApps } returns includedApps
        every { repository.appExits } returns appExits
        every { repository.appExitsEnabled } returns appExitsEnabled
        every { repository.vpnOnlyForCount } returns vpnOnlyForCount
        every { repository.lockedApps } returns lockedApps
        every { repository.routingSettings() } answers { settings() }
        val ops = slot<List<RoutingOp>>()
        every { repository.apply(capture(ops)) } answers
            {
                writes += ops.captured
                store(settings().apply(ops.captured))
            }
        val asked = slot<List<RoutingOp>>()
        every { repository.changeNarrowsTunnel(capture(asked)) } answers
            {
                changeNarrowsTunnel(settings(), settings().apply(asked.captured), ::installed)
            }
        every { preferences.showSystemAppsSplitTunneling() } returns showSystemApps
        every { relayProvider.catalogue } returns catalogue
    }

    @AfterEach
    fun tearDown() {
        testSubject.viewModelScope.coroutineContext.cancel()
        unmockkAll()
    }

    @Test
    fun `the list starts loading`() = runTest {
        initTestSubject()

        assertIs<Lc.Loading<Loading>>(testSubject.uiState.value)
    }

    @Test
    fun `a fresh install shows the VPN as the default and no rule`() = runTest {
        initTestSubject()

        testSubject.uiState.test {
            val state = awaitContent()
            assertEquals(DefaultRoute.Vpn, state.defaultRoute)
            assertEquals(emptyList(), state.rules)
            assertTrue(state.showNoRules)
            assertFalse(state.someAppOutside)
        }
    }

    @Test
    fun `the list shows each app with a rule once, by name, with the line of a country`() =
        runTest {
            store(
                AppRoutingSettings(
                    SplitTunnelMode.Exclude,
                    excludedApps = setOf("org.maps", "org.gone"),
                    appExitsEnabled = true,
                    appExits = mapOf("org.chat" to AppExit("ro"), "org.maps" to AppExit("nl")),
                )
            )
            appRoutes.value =
                listOf(AppRouteStatus(AppExit("ro"), AppRouteState.Connected, "192.0.2.4", listOf("org.chat")))
            initTestSubject()

            testSubject.uiState.test {
                val state = awaitContent()
                assertEquals(
                    listOf(
                        AppRuleItem(chat, AppRoute.Country(AppExit("ro")), AppRouteLine.Connected("192.0.2.4")),
                        AppRuleItem(maps, AppRoute.Direct, null),
                    ),
                    state.rules,
                )
                assertTrue(state.someAppOutside)
            }
        }

    @Test
    fun `an app sent outside the VPN turns bypass on, and back on the VPN turns it off`() = runTest {
        initTestSubject()

        testSubject.uiState.test {
            awaitContent()
            testSubject.onOpenApp(chat)
            val route = awaitPage<AppRoutingPage.Route>()
            assertEquals(AppRoute.Vpn, route.route)
            assertFalse(route.hasRule)

            testSubject.onChooseRoute(AppRoute.Direct)
            assertTrue(awaitPage<AppRoutingPage.Route>().hasRule)
            assertEquals(SplitTunnelMode.Exclude, splitMode.value)
            assertEquals(setOf(chat.packageName), excludedApps.value)

            testSubject.onRemoveRule()
            assertEquals(AppRoutingPage.Rules, awaitContent().page)
            assertEquals(SplitTunnelMode.Off, splitMode.value)
            assertEquals(emptySet(), excludedApps.value)
            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `a country is chosen on its page, and the route page shows it`() = runTest {
        catalogue.value = listOf(relay("de", "Berlin"), relay("ro", "Bucharest"))
        initTestSubject()

        testSubject.uiState.test {
            awaitContent()
            testSubject.onOpenApp(chat)
            testSubject.onOpenCountries()
            val picker = awaitPage<AppRoutingPage.Country>().picker
            assertEquals(listOf("DE", "RO"), picker.options.map { it.name })

            testSubject.onChooseExit(AppExit("ro", "Bucharest"))
            val route = awaitContentMatching { (it.page as? AppRoutingPage.Route)?.hasRule == true }
            assertEquals(
                AppRoute.Country(AppExit("ro", "Bucharest")),
                (route.page as AppRoutingPage.Route).route,
            )
            cancelAndIgnoreRemainingEvents()
        }
        assertTrue(appExitsEnabled.value)
        assertEquals(mapOf("org.chat" to AppExit("ro", "Bucharest")), appExits.value)
        coVerify { relayProvider.refreshIfStale() }
    }

    @Test
    fun `below Android 10 the country page never opens`() = runTest {
        initTestSubject(countrySupported = false)

        testSubject.uiState.test {
            awaitContent()
            testSubject.onOpenApp(chat)
            awaitPage<AppRoutingPage.Route>()
            testSubject.onOpenCountries()
            expectNoEvents()
        }
    }

    @Test
    fun `outside the VPN as the default keeps the countries and warns while every app is in the VPN`() =
        runTest {
            store(
                AppRoutingSettings(
                    SplitTunnelMode.Exclude,
                    excludedApps = setOf("org.bank"),
                )
            )
            initTestSubject()

            testSubject.uiState.test {
                awaitContent()
                testSubject.onChooseDefault(DefaultRoute.Direct)
                val state = awaitContent()
                assertEquals(DefaultRoute.Direct, state.defaultRoute)
                assertEquals(emptyList(), state.rules)
                assertTrue(state.fullTunnelFallback)
                assertNull(state.confirmation)
            }
            assertEquals(SplitTunnelMode.IncludeOnly, splitMode.value)
            assertEquals(emptySet(), excludedApps.value)
        }

    @Test
    fun `the first rule over the full tunnel fallback asks first, naming the app`() = runTest {
        store(AppRoutingSettings(SplitTunnelMode.IncludeOnly))
        initTestSubject()

        testSubject.uiState.test {
            awaitContent()
            testSubject.onOpenApp(bank)
            awaitPage<AppRoutingPage.Route>()
            testSubject.onChooseRoute(AppRoute.Vpn)
            assertEquals(NarrowingConfirmation(bank), awaitContent().confirmation)
            assertEquals(emptySet(), includedApps.value)

            testSubject.onCancelNarrowing()
            assertNull(awaitContent().confirmation)
            assertEquals(emptySet(), includedApps.value)

            testSubject.onChooseRoute(AppRoute.Vpn)
            assertEquals(NarrowingConfirmation(bank), awaitContent().confirmation)
            testSubject.onConfirmNarrowing()
            val state = expectMostRecentContent()
            assertNull(state.confirmation)
            assertEquals(listOf(AppRuleItem(bank, AppRoute.Vpn, null)), state.rules)
            assertFalse(state.fullTunnelFallback)
        }
        assertEquals(setOf(bank.packageName), includedApps.value)
    }

    @Test
    fun `outside the VPN as the default over apps with a country asks first`() = runTest {
        store(
            AppRoutingSettings(appExitsEnabled = true, appExits = mapOf("org.chat" to AppExit("nl")))
        )
        initTestSubject()

        testSubject.uiState.test {
            awaitContent()
            testSubject.onChooseDefault(DefaultRoute.Direct)
            assertEquals(NarrowingConfirmation(null), awaitContent().confirmation)
            assertEquals(SplitTunnelMode.Off, splitMode.value)

            testSubject.onConfirmNarrowing()
            assertEquals(DefaultRoute.Direct, expectMostRecentContent().defaultRoute)
        }
        assertEquals(SplitTunnelMode.IncludeOnly, splitMode.value)
    }

    @Test
    fun `a second rule with outside the VPN as the default asks nothing`() = runTest {
        store(AppRoutingSettings(SplitTunnelMode.IncludeOnly, includedApps = setOf("org.bank")))
        initTestSubject()

        testSubject.uiState.test {
            awaitContent()
            testSubject.onOpenApp(chat)
            testSubject.onChooseRoute(AppRoute.Vpn)
            assertNull(expectMostRecentContent().confirmation)
        }
        assertEquals(setOf(bank.packageName, chat.packageName), includedApps.value)
    }

    @Test
    fun `the add page lists the apps without a rule, narrowed by the search, system apps on demand`() =
        runTest {
            store(AppRoutingSettings(SplitTunnelMode.Exclude, excludedApps = setOf("org.bank")))
            initTestSubject()

            testSubject.uiState.test {
                awaitContent()
                testSubject.onOpenAddApp()
                assertEquals(listOf(chat, maps), awaitPage<AppRoutingPage.AddApp>().apps)

                testSubject.onAddAppSearchChange("ma")
                assertEquals(listOf(maps), awaitPage<AppRoutingPage.AddApp>().apps)

                testSubject.onAddAppSearchChange("")
                showSystemApps.value = true
                assertEquals(
                    listOf(chat, clock, maps),
                    expectMostRecentPage<AppRoutingPage.AddApp>().apps,
                )
            }
        }

    @Test
    fun `back goes from the countries to the route to the list, then leaves`() = runTest {
        initTestSubject()

        testSubject.uiState.test {
            awaitContent()
            testSubject.onOpenApp(chat)
            testSubject.onOpenCountries()
            awaitPage<AppRoutingPage.Country>()

            assertTrue(testSubject.onBack())
            awaitPage<AppRoutingPage.Route>()
            assertTrue(testSubject.onBack())
            assertEquals(AppRoutingPage.Rules, awaitContent().page)
            assertFalse(testSubject.onBack())
        }
    }

    @Test
    fun `an app picked from the add page gets no rule until a route other than the default`() =
        runTest {
            initTestSubject()

            testSubject.uiState.test {
                awaitContent()
                testSubject.onOpenAddApp()
                awaitPage<AppRoutingPage.AddApp>()
                testSubject.onOpenApp(maps)
                assertEquals(maps, awaitPage<AppRoutingPage.Route>().app)
                testSubject.onChooseRoute(AppRoute.Vpn)
                testSubject.onDone()
                assertEquals(emptyList(), awaitContent().rules)
            }
            assertEquals(emptyList(), writes)
        }

    private fun relay(country: String, city: String) =
        WarrenRelaySummary(
            exitId = "$country-$city",
            exitPubkeyHex = "",
            endpoint = "",
            country = country,
            city = city,
            active = true,
            weight = 1,
        )

    private suspend fun ReceiveTurbine<Lc<Loading, AppRoutingUiState>>.awaitContent():
        AppRoutingUiState {
        var item = awaitItem()
        while (item !is Lc.Content) item = awaitItem()
        return item.value
    }

    private suspend fun ReceiveTurbine<Lc<Loading, AppRoutingUiState>>.awaitContentMatching(
        predicate: (AppRoutingUiState) -> Boolean
    ): AppRoutingUiState {
        var state = awaitContent()
        while (!predicate(state)) state = awaitContent()
        return state
    }

    private suspend inline fun <reified P : AppRoutingPage> ReceiveTurbine<
        Lc<Loading, AppRoutingUiState>
    >
        .awaitPage(): P {
        var page = awaitContent().page
        while (page !is P) page = awaitContent().page
        return page
    }

    private fun ReceiveTurbine<Lc<Loading, AppRoutingUiState>>.expectMostRecentContent():
        AppRoutingUiState {
        val item = expectMostRecentItem()
        assertIs<Lc.Content<AppRoutingUiState>>(item)
        return item.value
    }

    @Test
    fun `an app locked on its route keeps its route and shows the lock on its rule`() = runTest {
        initTestSubject()

        testSubject.uiState.test {
            awaitItem()
            testSubject.onOpenApp(bank)
            testSubject.onSetLocked(true)

            assertEquals(listOf<RoutingOp>(RoutingOp.Lock("org.bank")), writes)
            val route = expectMostRecentPage<AppRoutingPage.Route>()
            assertTrue(route.locked && route.hasRule)
            testSubject.onDone()
            val rule = expectMostRecentContent().rules.single()
            assertEquals(AppRoute.Vpn, rule.route)
            assertTrue(rule.locked)
        }
    }

    @Test
    fun `a locked app is blocked while the VPN is off`() = runTest {
        lockedApps.value = setOf("org.bank")
        tunnelConnected.value = false
        initTestSubject()

        testSubject.uiState.test {
            assertEquals(AppRouteLine.Blocked, awaitContent().rules.single().line)
            tunnelConnected.value = true
            assertEquals(null, expectMostRecentContent().rules.single().line)
        }
    }

    @Test
    fun `an app outside the VPN is not locked`() = runTest {
        splitMode.value = SplitTunnelMode.IncludeOnly
        initTestSubject()

        testSubject.uiState.test {
            awaitItem()
            testSubject.onOpenApp(chat)
            testSubject.onSetLocked(true)

            assertEquals(emptyList<RoutingOp>(), writes)
            cancelAndIgnoreRemainingEvents()
        }
    }

    private inline fun <reified P : AppRoutingPage> ReceiveTurbine<
        Lc<Loading, AppRoutingUiState>
    >
        .expectMostRecentPage(): P = assertIs<P>(expectMostRecentContent().page)

    private fun initTestSubject(countrySupported: Boolean = true) {
        every { applicationsProvider.apps() } returns listOf(bank, chat, clock, maps)
        testSubject =
            SplitTunnelingViewModel(
                isModal = false,
                splitTunnelingRepository = repository,
                userPreferencesRepository = preferences,
                applicationsProvider = applicationsProvider,
                appRoutesStatusProvider = appRoutesProvider,
                relayProvider = relayProvider,
                countryPerAppSupported = countrySupported,
                dispatcher = UnconfinedTestDispatcher(),
                countryName = { it.uppercase() },
                tunnelConnected = tunnelConnected,
            )
    }
}
