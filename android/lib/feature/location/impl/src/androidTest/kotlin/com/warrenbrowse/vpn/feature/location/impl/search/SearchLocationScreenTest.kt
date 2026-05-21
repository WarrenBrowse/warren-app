package com.warrenbrowse.vpn.feature.location.impl.search

import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performTextInput
import de.mannodermaus.junit5.compose.ComposeContext
import io.mockk.MockKAnnotations
import io.mockk.mockk
import io.mockk.unmockkAll
import io.mockk.verify
import com.warrenbrowse.vpn.feature.location.api.LocationBottomSheetState
import com.warrenbrowse.vpn.feature.location.impl.data.DUMMY_RELAY_ITEM_CUSTOM_LISTS
import com.warrenbrowse.vpn.lib.common.Lce
import com.warrenbrowse.vpn.lib.model.CustomListId
import com.warrenbrowse.vpn.lib.model.RelayItem
import com.warrenbrowse.vpn.lib.model.RelayItemId
import com.warrenbrowse.vpn.lib.model.RelayListType
import com.warrenbrowse.vpn.lib.ui.component.relaylist.RelayListItem
import com.warrenbrowse.vpn.lib.ui.tag.SELECT_LOCATION_CUSTOM_LIST_HEADER_TEST_TAG
import com.warrenbrowse.vpn.screen.test.createEdgeToEdgeComposeExtension
import com.warrenbrowse.vpn.screen.test.setContentWithTheme
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.RegisterExtension

@OptIn(ExperimentalTestApi::class)
class SearchLocationScreenTest {
    @JvmField @RegisterExtension val composeExtension = createEdgeToEdgeComposeExtension()

    @BeforeEach
    fun setup() {
        MockKAnnotations.init(this)
    }

    @AfterEach
    fun teardown() {
        unmockkAll()
    }

    private fun ComposeContext.initScreen(
        state: Lce<Unit, SearchLocationUiState, Unit>,
        onUpdateBottomSheetState: (LocationBottomSheetState) -> Unit = {},
        onSelectRelayItem: (RelayItem, RelayListType) -> Unit = { _, _ -> },
        onToggleExpand: (RelayItemId, CustomListId?, Boolean) -> Unit = { _, _, _ -> },
        onSearchInputChanged: (String) -> Unit = {},
        onRemoveOwnershipFilter: () -> Unit = {},
        onRemoveProviderFilter: () -> Unit = {},
        onGoBack: () -> Unit = {},
    ) {
        setContentWithTheme {
            SearchLocationScreen(
                state = state,
                onSelectRelayItem = onSelectRelayItem,
                onToggleExpand = onToggleExpand,
                onSearchInputChanged = onSearchInputChanged,
                onRemoveOwnershipFilter = onRemoveOwnershipFilter,
                onRemoveProviderFilter = onRemoveProviderFilter,
                onGoBack = onGoBack,
                navigateToBottomSheet = onUpdateBottomSheetState,
            )
        }
    }

    @Test
    fun testSearchInput() = composeExtension.use {
        // Arrange
        val mockedSearchTermInput: (String) -> Unit = mockk(relaxed = true)
        initScreen(
            state =
                Lce.Content(
                    SearchLocationUiState(
                        searchTerm = "",
                        relayListType = RelayListType.Single,
                        filterChips = emptyList(),
                        relayListItems = emptyList(),
                        customLists = emptyList(),
                    )
                ),
            onSearchInputChanged = mockedSearchTermInput,
        )
        val mockSearchString = "SEARCH"

        // Act
        onNodeWithText("Search for...").performTextInput(mockSearchString)

        // Assert
        verify { mockedSearchTermInput.invoke(mockSearchString) }
    }

    @Test
    fun testSearchTermNotFound() = composeExtension.use {
        // Arrange
        val mockSearchString = "SEARCH"
        initScreen(
            state =
                Lce.Content(
                    SearchLocationUiState(
                        searchTerm = mockSearchString,
                        relayListType = RelayListType.Single,
                        filterChips = emptyList(),
                        relayListItems = listOf(RelayListItem.LocationsEmptyText(mockSearchString)),
                        customLists = emptyList(),
                    )
                )
        )

        // Assert
        onNodeWithText("No result for \"$mockSearchString\", please try a different search")
            .assertExists()
    }

    @Test
    fun givenNoCustomListsAndSearchIsActiveShouldNotShowCustomListHeader() = composeExtension.use {
        // Arrange
        val mockSearchString = "SEARCH"
        initScreen(
            state =
                Lce.Content(
                    SearchLocationUiState(
                        searchTerm = mockSearchString,
                        relayListType = RelayListType.Single,
                        filterChips = emptyList(),
                        relayListItems = emptyList(),
                        customLists = DUMMY_RELAY_ITEM_CUSTOM_LISTS,
                    )
                )
        )

        // Assert
        onNodeWithText(CUSTOM_LISTS_EMPTY_TEXT).assertDoesNotExist()
        onNodeWithTag(SELECT_LOCATION_CUSTOM_LIST_HEADER_TEST_TAG).assertDoesNotExist()
    }

    companion object {
        private const val CUSTOM_LISTS_EMPTY_TEXT = "To create a custom list press the \"︙\""
    }
}
