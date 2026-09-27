package com.warrenbrowse.vpn.feature.splittunneling.impl

import androidx.lifecycle.viewModelScope
import app.cash.turbine.test
import io.mockk.every
import io.mockk.mockk
import io.mockk.unmockkAll
import io.mockk.verify
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
import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.SplitTunnelingUseCase
import com.warrenbrowse.vpn.feature.splittunneling.impl.countries.CountryPerAppUiState
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.AppExit
import com.warrenbrowse.vpn.lib.model.AppRouteLine
import com.warrenbrowse.vpn.lib.model.AppRouteState
import com.warrenbrowse.vpn.lib.model.AppRouteStatus
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.model.SplitTunnelMode
import com.warrenbrowse.vpn.lib.repository.SplitTunnelingRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.lib.repository.WarrenAppRoutesStatusProvider
import com.warrenbrowse.vpn.lib.repository.WarrenRelayProvider
import com.warrenbrowse.vpn.lib.repository.WarrenRelaySummary
import io.mockk.coVerify
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.Timeout
import org.junit.jupiter.api.extension.ExtendWith

@ExperimentalCoroutinesApi
@ExtendWith(TestCoroutineRule::class)
@Timeout(3000L, unit = TimeUnit.MILLISECONDS)
class SplitTunnelingViewModelTest {

    private val mockedApplicationsProvider = mockk<ApplicationsProvider>()
    private val mockedSplitTunnelingRepository = mockk<SplitTunnelingRepository>(relaxed = true)
    private val mockedUserPreferencesRepository = mockk<UserPreferencesRepository>()
    private lateinit var testSubject: SplitTunnelingViewModel

    private val splitMode = MutableStateFlow(SplitTunnelMode.Off)
    private val excludedApps: MutableStateFlow<Set<PackageName>> = MutableStateFlow(emptySet())
    private val includedApps: MutableStateFlow<Set<PackageName>> = MutableStateFlow(emptySet())
    private val showSystemApps: MutableStateFlow<Boolean> = MutableStateFlow(false)
    private val appExits = MutableStateFlow<Map<String, AppExit>>(emptyMap())
    private val appExitsEnabled = MutableStateFlow(false)
    private val appRoutes = MutableStateFlow<List<AppRouteStatus>>(emptyList())
    private val catalogue = MutableStateFlow<List<WarrenRelaySummary>>(emptyList())
    private val appRoutesProvider =
        object : WarrenAppRoutesStatusProvider {
            override val appRoutes = this@SplitTunnelingViewModelTest.appRoutes
        }
    private val mockedRelayProvider = mockk<WarrenRelayProvider>(relaxed = true)

    private val chat = AppData(PackageName("org.chat"), 0, "Chat")
    private val bank = AppData(PackageName("org.bank"), 0, "Bank")
    private val maps = AppData(PackageName("org.maps"), 0, "Maps")

    @BeforeEach
    fun setup() {
        every { mockedSplitTunnelingRepository.splitMode } returns splitMode
        every { mockedSplitTunnelingRepository.excludedApps } returns excludedApps
        every { mockedSplitTunnelingRepository.includedApps } returns includedApps
        every { mockedSplitTunnelingRepository.setSplitMode(any()) } answers
            {
                splitMode.value = firstArg()
            }
        every { mockedUserPreferencesRepository.showSystemAppsSplitTunneling() } returns
            showSystemApps
        every { mockedSplitTunnelingRepository.appExits } returns appExits
        every { mockedSplitTunnelingRepository.appExitsEnabled } returns appExitsEnabled
        every { mockedRelayProvider.catalogue } returns catalogue
    }

    @AfterEach
    fun tearDown() {
        testSubject.viewModelScope.coroutineContext.cancel()
        unmockkAll()
    }

    @Test
    fun `initial state should be loading`() = runTest {
        initTestSubject(emptyList())

        assertIs<Lc.Loading<Loading>>(testSubject.uiState.value)
    }

    @Test
    fun `the bypass tab lists the excluded apps as chosen`() = runTest {
        excludedApps.value = setOf(chat.packageName)
        includedApps.value = setOf(bank.packageName)
        initTestSubject(listOf(bank, chat, maps))

        testSubject.uiState.test {
            val state = awaitContent()
            assertEquals(SplitTunnelingTab.Bypass, state.tab)
            assertEquals(listOf(chat), state.selectedApps)
            assertEquals(listOf(bank, maps), state.otherApps)
        }
    }

    @Test
    fun `the screen opens on vpn only for while include-only is on and lists its apps`() =
        runTest {
            splitMode.value = SplitTunnelMode.IncludeOnly
            excludedApps.value = setOf(chat.packageName)
            includedApps.value = setOf(bank.packageName)
            initTestSubject(listOf(bank, chat, maps))

            testSubject.uiState.test {
                val state = awaitContent()
                assertEquals(SplitTunnelingTab.IncludeOnly, state.tab)
                assertTrue(state.tabModeOn)
                assertEquals(listOf(bank), state.selectedApps)
                assertEquals(listOf(chat, maps), state.otherApps)
            }
        }

    @Test
    fun `an app tapped in a tab is added to or removed from that tab's list`() = runTest {
        includedApps.value = setOf(bank.packageName)
        initTestSubject(listOf(bank, chat))

        testSubject.onAddAppClick(chat.packageName)
        testSubject.onSelectTab(SplitTunnelingTab.IncludeOnly)
        testSubject.onAddAppClick(chat.packageName)
        testSubject.onRemoveAppClick(bank.packageName)

        verify { mockedSplitTunnelingRepository.addExcludedApp(chat.packageName) }
        verify { mockedSplitTunnelingRepository.addIncludedApp(chat.packageName) }
        verify { mockedSplitTunnelingRepository.removeIncludedApp(bank.packageName) }
        verify(exactly = 0) { mockedSplitTunnelingRepository.addIncludedApp(bank.packageName) }
    }

    @Test
    fun `turning vpn only for on waits for the confirmation`() = runTest {
        initTestSubject(listOf(bank))
        testSubject.onSelectTab(SplitTunnelingTab.IncludeOnly)

        testSubject.uiState.test {
            awaitContent()
            testSubject.onSplitModeSwitch(true)
            assertEquals(
                ModeChangeConfirmation(leavesDeviceUnprotected = true, replaces = null),
                awaitContent().confirmation,
            )
            verify(exactly = 0) { mockedSplitTunnelingRepository.setSplitMode(any()) }

            testSubject.onConfirmModeChange()
            verify { mockedSplitTunnelingRepository.setSplitMode(SplitTunnelMode.IncludeOnly) }
            val applied = expectMostRecentContent()
            assertNull(applied.confirmation)
            assertEquals(SplitTunnelMode.IncludeOnly, applied.splitMode)
        }
    }

    @Test
    fun `a cancelled confirmation leaves the mode as it was`() = runTest {
        splitMode.value = SplitTunnelMode.IncludeOnly
        initTestSubject(listOf(bank))
        testSubject.onSelectTab(SplitTunnelingTab.Bypass)

        testSubject.uiState.test {
            awaitContent()
            testSubject.onSplitModeSwitch(true)
            assertEquals(SplitTunnelMode.IncludeOnly, awaitContent().confirmation?.replaces)

            testSubject.onCancelModeChange()
            assertNull(awaitContent().confirmation)
            verify(exactly = 0) { mockedSplitTunnelingRepository.setSplitMode(any()) }
        }
    }

    @Test
    fun `a change that needs no confirmation applies at once`() = runTest {
        initTestSubject(listOf(bank))

        testSubject.onSplitModeSwitch(true)
        testSubject.onSelectTab(SplitTunnelingTab.IncludeOnly)
        splitMode.value = SplitTunnelMode.IncludeOnly
        testSubject.onSplitModeSwitch(false)

        verify { mockedSplitTunnelingRepository.setSplitMode(SplitTunnelMode.Exclude) }
        verify { mockedSplitTunnelingRepository.setSplitMode(SplitTunnelMode.Off) }
    }

    @Test
    fun `include-only with no chosen app on the device is named`() = runTest {
        splitMode.value = SplitTunnelMode.IncludeOnly
        includedApps.value = setOf(PackageName("org.uninstalled"))
        initTestSubject(listOf(bank))

        testSubject.uiState.test {
            assertTrue(awaitContent().includeOnlyWithoutApps)
            includedApps.value = setOf(bank.packageName)
            assertFalse(awaitContent().includeOnlyWithoutApps)
        }
    }

    @Test
    fun `the connect screen chip opens the country tab whatever the mode`() = runTest {
        splitMode.value = SplitTunnelMode.IncludeOnly
        initTestSubject(listOf(bank), initialTab = SplitTunnelingTab.CountryPerApp)

        testSubject.uiState.test {
            val state = awaitContent()
            assertEquals(SplitTunnelingTab.CountryPerApp, state.tab)
            assertFalse(state.tabModeOn)
        }
    }

    @Test
    fun `the country tab lists the apps with a country apart, by name, each with its line`() =
        runTest {
            appExitsEnabled.value = true
            appExits.value = mapOf(maps.packageName.value to AppExit("se"), chat.packageName.value to AppExit("de", "Berlin"))
            appRoutes.value =
                listOf(
                    AppRouteStatus(AppExit("se"), AppRouteState.Connected, "198.51.100.7", listOf(maps.packageName.value)),
                )
            initTestSubject(listOf(bank, chat, maps), initialTab = SplitTunnelingTab.CountryPerApp)

            testSubject.uiState.test {
                val countries = awaitCountries()
                assertEquals(listOf(chat, maps), countries.withCountry.map { it.app })
                assertEquals(
                    listOf(AppRouteLine.Waiting, AppRouteLine.Connected("198.51.100.7")),
                    countries.withCountry.map { it.line },
                )
                assertEquals(AppExit("de", "Berlin"), countries.withCountry.first().exit)
                assertEquals(listOf(bank), countries.otherApps)
            }
        }

    @Test
    fun `a saved country with the switch off reads as paused`() = runTest {
        appExits.value = mapOf(chat.packageName.value to AppExit("se"))
        initTestSubject(listOf(chat), initialTab = SplitTunnelingTab.CountryPerApp)

        testSubject.uiState.test {
            assertEquals(AppRouteLine.Paused, awaitCountries().withCountry.single().line)
        }
    }

    @Test
    fun `the country search narrows both sections`() = runTest {
        appExits.value = mapOf(chat.packageName.value to AppExit("se"), maps.packageName.value to AppExit("fi"))
        initTestSubject(listOf(bank, chat, maps), initialTab = SplitTunnelingTab.CountryPerApp)

        testSubject.onCountrySearchChange("A")
        testSubject.uiState.test {
            val countries = awaitCountries()
            assertEquals(listOf(chat, maps), countries.withCountry.map { it.app })
            assertEquals(listOf(bank), countries.otherApps)
            testSubject.onCountrySearchChange("ma")
            val narrowed = awaitCountries()
            assertEquals(listOf(maps), narrowed.withCountry.map { it.app })
            assertEquals(emptyList(), narrowed.otherApps)
            testSubject.onCountrySearchChange("zzz")
            assertTrue(awaitCountries().noSearchResult)
        }
    }

    @Test
    fun `choosing in the picker saves the country, closes the picker and moves nothing else`() =
        runTest {
            appExits.value = mapOf(chat.packageName.value to AppExit("de"))
            catalogue.value = listOf(relay("de", "Berlin"), relay("se", "Stockholm"))
            initTestSubject(listOf(bank, chat), initialTab = SplitTunnelingTab.CountryPerApp)

            testSubject.uiState.test {
                awaitCountries()
                testSubject.onPickCountry(chat)
                val picker = awaitCountries().picker!!
                assertEquals(AppExit("de"), picker.current)
                assertEquals(setOf("de"), picker.expanded)
                assertEquals(listOf("DE", "SE"), picker.options.map { it.name })

                testSubject.onChooseExit(AppExit("se", "Stockholm"))
                assertNull(awaitCountries().picker)
            }
            verify { mockedSplitTunnelingRepository.setAppExit(chat.packageName, AppExit("se", "Stockholm")) }
            coVerify { mockedRelayProvider.refreshIfStale() }
            verify(exactly = 0) { mockedSplitTunnelingRepository.setSplitMode(any()) }
        }

    @Test
    fun `a first country that would narrow the full tunnel waits for the confirmation`() =
        runTest {
            splitMode.value = SplitTunnelMode.IncludeOnly
            every {
                mockedSplitTunnelingRepository.countryChoiceNarrowsFullTunnel(chat.packageName, any())
            } returns true
            initTestSubject(listOf(bank, chat), initialTab = SplitTunnelingTab.CountryPerApp)

            testSubject.uiState.test {
                awaitCountries()
                testSubject.onPickCountry(chat)
                awaitCountries()
                testSubject.onChooseExit(AppExit("se"))
                val asking = expectMostRecentContent().countryPerApp!!
                assertNull(asking.picker)
                assertEquals(chat, asking.onlyAppConfirmation)
                verify(exactly = 0) { mockedSplitTunnelingRepository.setAppExit(any(), any()) }

                testSubject.onConfirmOnlyApp()
                assertNull(expectMostRecentContent().countryPerApp!!.onlyAppConfirmation)
            }
            verify { mockedSplitTunnelingRepository.setAppExit(chat.packageName, AppExit("se")) }
        }

    @Test
    fun `a cancelled first country leaves the settings untouched`() = runTest {
        splitMode.value = SplitTunnelMode.IncludeOnly
        every {
            mockedSplitTunnelingRepository.countryChoiceNarrowsFullTunnel(any(), any())
        } returns true
        initTestSubject(listOf(chat), initialTab = SplitTunnelingTab.CountryPerApp)

        testSubject.uiState.test {
            awaitCountries()
            testSubject.onPickCountry(chat)
            testSubject.onChooseExit(AppExit("se"))
            assertEquals(chat, expectMostRecentContent().countryPerApp!!.onlyAppConfirmation)

            testSubject.onCancelOnlyApp()
            assertNull(expectMostRecentContent().countryPerApp!!.onlyAppConfirmation)
        }
        verify(exactly = 0) { mockedSplitTunnelingRepository.setAppExit(any(), any()) }
        verify(exactly = 0) { mockedSplitTunnelingRepository.setAppExitsEnabled(any()) }
    }

    @Test
    fun `removing a country from the row or the picker clears it`() = runTest {
        appExits.value = mapOf(chat.packageName.value to AppExit("de"), bank.packageName.value to AppExit("se"))
        initTestSubject(listOf(bank, chat), initialTab = SplitTunnelingTab.CountryPerApp)

        testSubject.onClearAppCountry(bank.packageName)
        testSubject.onPickCountry(chat)
        testSubject.onRemovePickedCountry()

        verify { mockedSplitTunnelingRepository.clearAppExit(bank.packageName) }
        verify { mockedSplitTunnelingRepository.clearAppExit(chat.packageName) }
    }

    @Test
    fun `the country switch never asks for a confirmation and leaves the split mode alone`() =
        runTest {
            splitMode.value = SplitTunnelMode.IncludeOnly
            initTestSubject(listOf(bank), initialTab = SplitTunnelingTab.CountryPerApp)

            testSubject.uiState.test {
                awaitContent()
                testSubject.onAppExitsSwitch(true)
                testSubject.onSplitModeSwitch(true)
                // A pending confirmation would have produced a new state.
                expectNoEvents()
            }
            verify { mockedSplitTunnelingRepository.setAppExitsEnabled(true) }
            verify(exactly = 0) { mockedSplitTunnelingRepository.setSplitMode(any()) }
        }

    @Test
    fun `below Android 10 the country tab cannot be turned on nor a country chosen`() = runTest {
        initTestSubject(
            listOf(bank),
            initialTab = SplitTunnelingTab.CountryPerApp,
            countryPerAppSupported = false,
        )

        testSubject.uiState.test {
            assertFalse(awaitCountries().supported)
            testSubject.onAppExitsSwitch(true)
            testSubject.onPickCountry(bank)
            // An open picker would have produced a new state.
            expectNoEvents()
        }
        verify(exactly = 0) { mockedSplitTunnelingRepository.setAppExitsEnabled(any()) }
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

    private suspend fun app.cash.turbine.ReceiveTurbine<Lc<Loading, SplitTunnelingUiState>>
        .awaitCountries(): CountryPerAppUiState {
        var countries = awaitContent().countryPerApp
        while (countries == null) countries = awaitContent().countryPerApp
        return countries
    }

    private suspend fun app.cash.turbine.ReceiveTurbine<Lc<Loading, SplitTunnelingUiState>>
        .awaitContent(): SplitTunnelingUiState {
        var item = awaitItem()
        while (item !is Lc.Content) item = awaitItem()
        return item.value
    }

    private fun app.cash.turbine.ReceiveTurbine<Lc<Loading, SplitTunnelingUiState>>
        .expectMostRecentContent(): SplitTunnelingUiState {
        val item = expectMostRecentItem()
        assertIs<Lc.Content<SplitTunnelingUiState>>(item)
        return item.value
    }

    private fun initTestSubject(
        appList: List<AppData>,
        initialTab: SplitTunnelingTab? = null,
        countryPerAppSupported: Boolean = true,
    ) {
        every { mockedApplicationsProvider.apps() } returns appList
        testSubject =
            SplitTunnelingViewModel(
                isModal = false,
                initialTab = initialTab,
                mockedSplitTunnelingRepository,
                mockedUserPreferencesRepository,
                SplitTunnelingUseCase(
                    mockedSplitTunnelingRepository,
                    mockedApplicationsProvider,
                    mockedUserPreferencesRepository,
                    UnconfinedTestDispatcher(),
                ),
                appRoutesProvider,
                mockedRelayProvider,
                countryPerAppSupported = countryPerAppSupported,
                UnconfinedTestDispatcher(),
                countryName = { it.uppercase() },
            )
    }
}
