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
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.model.SplitTunnelMode
import com.warrenbrowse.vpn.lib.repository.SplitTunnelingRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
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

    private fun initTestSubject(appList: List<AppData>) {
        every { mockedApplicationsProvider.apps() } returns appList
        testSubject =
            SplitTunnelingViewModel(
                isModal = false,
                mockedSplitTunnelingRepository,
                mockedUserPreferencesRepository,
                SplitTunnelingUseCase(
                    mockedSplitTunnelingRepository,
                    mockedApplicationsProvider,
                    mockedUserPreferencesRepository,
                    UnconfinedTestDispatcher(),
                ),
                UnconfinedTestDispatcher(),
            )
    }
}
