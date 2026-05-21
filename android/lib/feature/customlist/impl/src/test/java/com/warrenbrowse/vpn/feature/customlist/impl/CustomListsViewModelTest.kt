package com.warrenbrowse.vpn.feature.customlist.impl

import app.cash.turbine.test
import io.mockk.coVerify
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest
import com.warrenbrowse.vpn.feature.customlist.impl.screen.lists.CustomListsUiState
import com.warrenbrowse.vpn.feature.customlist.impl.screen.lists.CustomListsViewModel
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.CustomList
import com.warrenbrowse.vpn.lib.model.communication.CustomListAction
import com.warrenbrowse.vpn.lib.repository.CustomListsRepository
import com.warrenbrowse.vpn.lib.usecase.customlists.CustomListActionUseCase
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith

@ExtendWith(TestCoroutineRule::class)
class CustomListsViewModelTest {
    private val mockCustomListsRepository: CustomListsRepository = mockk(relaxed = true)
    private val mockCustomListsActionUseCase: CustomListActionUseCase = mockk(relaxed = true)

    @Test
    fun `given custom list from relay list use case should be in state`() = runTest {
        // Arrange
        val customLists: List<CustomList> = listOf(mockk())
        val expectedState = CustomListsUiState.Content(customLists)
        every { mockCustomListsRepository.customLists } returns MutableStateFlow(customLists)
        val viewModel = createViewModel()

        // Act, Assert
        viewModel.uiState.test { assertEquals(expectedState, awaitItem()) }
    }

    @Test
    fun `undo delete action should call custom list use case`() = runTest {
        // Arrange
        val viewModel = createViewModel()
        val action: CustomListAction.Create = mockk()

        // Act
        viewModel.undoDeleteCustomList(action)

        // Assert
        coVerify { mockCustomListsActionUseCase(action) }
    }

    private fun createViewModel() =
        CustomListsViewModel(
            customListsRepository = mockCustomListsRepository,
            customListActionUseCase = mockCustomListsActionUseCase,
        )
}
