package com.warrenbrowse.vpn.feature.customlist.impl

import app.cash.turbine.test
import io.mockk.every
import io.mockk.mockk
import kotlin.test.assertIs
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest
import com.warrenbrowse.vpn.feature.customlist.impl.screen.editlist.EditCustomListUiState
import com.warrenbrowse.vpn.feature.customlist.impl.screen.editlist.EditCustomListViewModel
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.CustomList
import com.warrenbrowse.vpn.lib.model.CustomListId
import com.warrenbrowse.vpn.lib.model.CustomListName
import com.warrenbrowse.vpn.lib.repository.CustomListsRepository
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith

@ExtendWith(TestCoroutineRule::class)
class EditCustomListViewModelTest {
    private val mockCustomListsRepository: CustomListsRepository = mockk(relaxed = true)

    @Test
    fun `given a custom list id that does not exists should return not found ui state`() = runTest {
        // Arrange
        val customListId = CustomListId("2")
        val name = CustomListName.fromString("test")
        val customList = CustomList(id = CustomListId("1"), name = name, locations = emptyList())
        every { mockCustomListsRepository.customLists } returns MutableStateFlow(listOf(customList))
        val viewModel = createViewModel(customListId)

        // Act, Assert
        viewModel.uiState.test {
            val item = awaitItem()
            assertIs<EditCustomListUiState.NotFound>(item)
        }
    }

    @Test
    fun `given a custom list id that exists should return content ui state`() = runTest {
        // Arrange
        val customListId = CustomListId("1")
        val name = CustomListName.fromString("test")
        val customList = CustomList(id = customListId, name = name, locations = emptyList())
        every { mockCustomListsRepository.customLists } returns MutableStateFlow(listOf(customList))
        val viewModel = createViewModel(customListId)

        // Act, Assert
        viewModel.uiState.test {
            val item = awaitItem()
            assertIs<EditCustomListUiState.Content>(item)
            assertEquals(item.id, customList.id)
            assertEquals(item.name, customList.name)
            assertEquals(item.locations, customList.locations)
        }
    }

    private fun createViewModel(customListId: CustomListId) =
        EditCustomListViewModel(
            customListId = customListId,
            customListsRepository = mockCustomListsRepository,
        )
}
