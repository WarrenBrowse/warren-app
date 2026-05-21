package com.warrenbrowse.vpn.feature.location.impl.list

import app.cash.turbine.test
import io.mockk.every
import io.mockk.mockk
import io.mockk.mockkStatic
import io.mockk.unmockkAll
import io.mockk.verify
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest
import com.warrenbrowse.vpn.feature.location.impl.RelayListScrollConnection
import com.warrenbrowse.vpn.feature.location.impl.search.relayListItems
import com.warrenbrowse.vpn.lib.common.Lce
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.common.test.assertLists
import com.warrenbrowse.vpn.lib.common.util.entryBlocked
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.GeoLocationId
import com.warrenbrowse.vpn.lib.model.MultihopRelayListType
import com.warrenbrowse.vpn.lib.model.RelayItem
import com.warrenbrowse.vpn.lib.model.RelayItemSelection
import com.warrenbrowse.vpn.lib.model.RelayListType
import com.warrenbrowse.vpn.lib.model.Settings
import com.warrenbrowse.vpn.lib.repository.RelayListRepository
import com.warrenbrowse.vpn.lib.repository.SettingsRepository
import com.warrenbrowse.vpn.lib.repository.WireguardConstraintsRepository
import com.warrenbrowse.vpn.lib.ui.component.relaylist.RelayListItem
import com.warrenbrowse.vpn.lib.usecase.FilteredRelayListUseCase
import com.warrenbrowse.vpn.lib.usecase.RecentsUseCase
import com.warrenbrowse.vpn.lib.usecase.SelectedLocationUseCase
import com.warrenbrowse.vpn.lib.usecase.customlists.CustomListsRelayItemUseCase
import com.warrenbrowse.vpn.lib.usecase.customlists.FilterCustomListsRelayItemUseCase
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith

@ExtendWith(TestCoroutineRule::class)
class SelectLocationListViewModelTest {

    private val mockFilteredRelayListUseCase: FilteredRelayListUseCase = mockk()
    private val mockFilteredCustomListRelayItemsUseCase: FilterCustomListsRelayItemUseCase = mockk()
    private val mockSelectedLocationUseCase: SelectedLocationUseCase = mockk()
    private val mockWireguardConstraintsRepository: WireguardConstraintsRepository = mockk()
    private val mockRelayListRepository: RelayListRepository = mockk()
    private val mockCustomListRelayItemsUseCase: CustomListsRelayItemUseCase = mockk()
    private val mockSettingsRepository: SettingsRepository = mockk()
    private val mockRecentsUseCase: RecentsUseCase = mockk()

    private val relayListScrollConnection: RelayListScrollConnection = RelayListScrollConnection()

    private val filteredRelayList = MutableStateFlow<List<RelayItem.Location.Country>>(emptyList())
    private val selectedLocationFlow = MutableStateFlow<RelayItemSelection>(mockk(relaxed = true))
    private val filteredCustomListRelayItems =
        MutableStateFlow<List<RelayItem.CustomList>>(emptyList())
    private val customListRelayItems = MutableStateFlow<List<RelayItem.CustomList>>(emptyList())
    private val recentsRelayItems = MutableStateFlow<List<RelayItem>?>(emptyList())
    private val settings = MutableStateFlow(mockk<Settings>(relaxed = true))

    private lateinit var viewModel: SelectLocationListViewModel

    @BeforeEach
    fun setUp() {
        // Used for initial selection
        every { mockRelayListRepository.selectedLocation } returns MutableStateFlow(Constraint.Any)
        every { mockWireguardConstraintsRepository.wireguardConstraints } returns
            MutableStateFlow(null)

        every { mockSelectedLocationUseCase() } returns selectedLocationFlow
        every { mockFilteredRelayListUseCase(any()) } returns filteredRelayList
        every { mockFilteredCustomListRelayItemsUseCase(any()) } returns
            filteredCustomListRelayItems
        every { mockCustomListRelayItemsUseCase() } returns customListRelayItems
        every { mockSettingsRepository.settingsUpdates } returns settings
        every { mockRecentsUseCase(any()) } returns recentsRelayItems

        mockkStatic(RELAY_ITEM_LIST_CREATOR_CLASS)
        mockkStatic(LOCATION_UTIL_CLASS)
    }

    @AfterEach
    fun tearDown() {
        unmockkAll()
    }

    @Test
    fun `initial state should be loading`() = runTest {
        // Arrange
        viewModel = createSelectLocationListViewModel(relayListType = RelayListType.Single)

        // Assert
        assertEquals(Lce.Loading(Unit), viewModel.uiState.value)
    }

    @Test
    fun `given filteredRelayList emits update uiState should contain new update`() = runTest {
        // Arrange
        viewModel = createSelectLocationListViewModel(RelayListType.Single)
        filteredRelayList.value = testCountries
        val selectedId = testCountries.first().id
        selectedLocationFlow.value = RelayItemSelection.Single(Constraint.Only(selectedId))

        // Act, Assert
        viewModel.uiState.test {
            val actualState = awaitItem()
            assertIs<Lce.Content<SelectLocationListUiState>>(actualState)
            assertLists(
                testCountries.map { it.id },
                actualState.value.relayListItems.mapNotNull { it.relayItemId() },
            )
            assertTrue(
                actualState.value.relayListItems
                    .filterIsInstance<RelayListItem.SelectableItem>()
                    .first { it.relayItemId() == selectedId }
                    .isSelected
            )
        }
    }

    @Test
    fun `given relay is not selected all relay items should not be selected`() = runTest {
        // Arrange
        viewModel = createSelectLocationListViewModel(RelayListType.Single)
        filteredRelayList.value = testCountries
        selectedLocationFlow.value = RelayItemSelection.Single(Constraint.Any)

        // Act, Assert
        viewModel.uiState.test {
            val actualState = awaitItem()
            assertIs<Lce.Content<SelectLocationListUiState>>(actualState)
            assertLists(
                testCountries.map { it.id },
                actualState.value.relayListItems.mapNotNull { it.relayItemId() },
            )
            assertTrue(
                actualState.value.relayListItems
                    .filterIsInstance<RelayListItem.SelectableItem>()
                    .all { !it.isSelected }
            )
        }
    }

    @Test
    fun `given relay list type exit and entry blocked no item should be selected in the entry list`() =
        runTest {
            // Arrange
            viewModel =
                createSelectLocationListViewModel(
                    RelayListType.Multihop(MultihopRelayListType.EXIT)
                )
            filteredRelayList.value = testCountries
            val exitLocation = Constraint.Only(GeoLocationId.Country("us"))
            selectedLocationFlow.value =
                RelayItemSelection.Multiple(
                    entryLocation = Constraint.Only(GeoLocationId.Country("se")),
                    exitLocation = exitLocation,
                )
            every { settings.value.entryBlocked() } returns true

            // Act, Assert
            viewModel.uiState.test {
                awaitItem()

                verify {
                    relayListItems(
                        relayListType = RelayListType.Multihop(MultihopRelayListType.EXIT),
                        relayCountries = testCountries,
                        customLists = any(),
                        recents = any(),
                        selectedItem = any(),
                        selectedByThisEntryExitList = exitLocation.getOrNull(),
                        selectedByOtherEntryExitList = null,
                        expandedItems = emptySet(),
                    )
                }
            }
        }

    @Test
    fun `given relay type entry list and entry blocked uiState should be error`() = runTest {
        // Arrange
        viewModel =
            createSelectLocationListViewModel(RelayListType.Multihop(MultihopRelayListType.ENTRY))
        filteredRelayList.value = testCountries
        selectedLocationFlow.value = RelayItemSelection.Multiple(Constraint.Any, Constraint.Any)
        val mockSettings: Settings = mockk()
        every { mockSettings.entryBlocked() } returns true
        settings.value = mockSettings

        // Act, Assert
        viewModel.uiState.test {
            val actualState = awaitItem()
            assertIs<Lce.Error<Unit>>(actualState)
        }
    }

    @Test
    fun `given relay type exit list and entry blocked should work`() = runTest {
        // Arrange
        viewModel =
            createSelectLocationListViewModel(RelayListType.Multihop(MultihopRelayListType.EXIT))
        filteredRelayList.value = testCountries
        selectedLocationFlow.value = RelayItemSelection.Multiple(Constraint.Any, Constraint.Any)
        val mockSettings: Settings = mockk()
        every { mockSettings.entryBlocked() } returns true
        settings.value = mockSettings

        // Act, Assert
        viewModel.uiState.test {
            val actualState = awaitItem()
            assertIs<Lce.Content<SelectLocationListUiState>>(actualState)
            assertLists(
                testCountries.map { it.id },
                actualState.value.relayListItems.mapNotNull { it.relayItemId() },
            )
        }
    }

    @Test
    fun `given relay type single list and entry blocked should work`() = runTest {
        // Arrange
        viewModel = createSelectLocationListViewModel(RelayListType.Single)
        filteredRelayList.value = testCountries
        selectedLocationFlow.value = RelayItemSelection.Multiple(Constraint.Any, Constraint.Any)
        val mockSettings: Settings = mockk()
        every { mockSettings.entryBlocked() } returns true
        settings.value = mockSettings

        // Act, Assert
        viewModel.uiState.test {
            val actualState = awaitItem()
            assertIs<Lce.Content<SelectLocationListUiState>>(actualState)
            assertLists(
                testCountries.map { it.id },
                actualState.value.relayListItems.mapNotNull { it.relayItemId() },
            )
        }
    }

    private fun createSelectLocationListViewModel(relayListType: RelayListType) =
        SelectLocationListViewModel(
            relayListType = relayListType,
            filteredRelayListUseCase = mockFilteredRelayListUseCase,
            filteredCustomListRelayItemsUseCase = mockFilteredCustomListRelayItemsUseCase,
            selectedLocationUseCase = mockSelectedLocationUseCase,
            wireguardConstraintsRepository = mockWireguardConstraintsRepository,
            relayListRepository = mockRelayListRepository,
            settingsRepository = mockSettingsRepository,
            relayListScrollConnection = relayListScrollConnection,
            recentsUseCase = mockRecentsUseCase,
        )

    private fun RelayListItem.relayItemId() =
        when (this) {
            is RelayListItem.CustomListEntryItem -> item.id
            is RelayListItem.CustomListItem -> item.id
            is RelayListItem.GeoLocationItem -> item.id
            is RelayListItem.RecentListItem -> item.id
            is RelayListItem.CustomListFooter,
            is RelayListItem.LocationsEmptyText,
            is RelayListItem.EmptyRelayList,
            is RelayListItem.SectionDivider,
            is RelayListItem.CustomListHeader,
            RelayListItem.LocationHeader,
            RelayListItem.RecentsListHeader,
            RelayListItem.RecentsListFooter -> null
        }

    companion object {
        private const val RELAY_ITEM_LIST_CREATOR_CLASS =
            "com.warrenbrowse.vpn.feature.location.impl.search.RelayItemListCreatorKt"
        private const val LOCATION_UTIL_CLASS =
            "com.warrenbrowse.vpn.lib.common.util.LocationUtilKt"

        private val testCountries =
            listOf(
                RelayItem.Location.Country(
                    id = GeoLocationId.Country("se"),
                    "Sweden",
                    listOf(
                        RelayItem.Location.City(
                            id = GeoLocationId.City(GeoLocationId.Country("se"), "got"),
                            "Gothenburg",
                            emptyList(),
                            countryName = "Sweden",
                        )
                    ),
                ),
                RelayItem.Location.Country(id = GeoLocationId.Country("no"), "Norway", emptyList()),
            )
    }
}
