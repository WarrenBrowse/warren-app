package com.warrenbrowse.vpn.feature.customlist.impl

import app.cash.turbine.test
import arrow.core.left
import arrow.core.right
import io.mockk.coEvery
import io.mockk.every
import io.mockk.mockk
import kotlin.test.assertIs
import kotlinx.coroutines.test.runTest
import com.warrenbrowse.vpn.feature.customlist.api.EditCustomListNameNavKey
import com.warrenbrowse.vpn.feature.customlist.impl.screen.editname.EditCustomListNameDialogSideEffect
import com.warrenbrowse.vpn.feature.customlist.impl.screen.editname.EditCustomListNameDialogViewModel
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.CustomListId
import com.warrenbrowse.vpn.lib.model.CustomListName
import com.warrenbrowse.vpn.lib.model.NameAlreadyExists
import com.warrenbrowse.vpn.lib.model.communication.CustomListAction
import com.warrenbrowse.vpn.lib.model.communication.CustomListActionResultData
import com.warrenbrowse.vpn.lib.model.communication.Renamed
import com.warrenbrowse.vpn.lib.usecase.customlists.CustomListActionUseCase
import com.warrenbrowse.vpn.lib.usecase.customlists.RenameError
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith

@ExtendWith(TestCoroutineRule::class)
class EditCustomListNameDialogViewModelTest {
    private val mockCustomListActionUseCase: CustomListActionUseCase = mockk()

    @Test
    fun `when successfully renamed list should emit return with result side effect`() = runTest {
        // Arrange
        val renamed: Renamed = mockk()
        val customListId = CustomListId("id")
        val customListName = CustomListName.fromString("list")
        val undo: CustomListAction.Rename = mockk()
        val expectedResult =
            CustomListActionResultData.Success.Renamed(newName = customListName, undo = undo)
        every { renamed.name } returns customListName
        every { renamed.undo } returns undo
        val viewModel = createViewModel(customListId, customListName.value)
        coEvery { mockCustomListActionUseCase(any<CustomListAction.Rename>()) } returns
            renamed.right()

        // Act, Assert
        viewModel.uiSideEffect.test {
            viewModel.updateCustomListName(customListName.value)
            val sideEffect = awaitItem()
            assertIs<EditCustomListNameDialogSideEffect.ReturnWithResult>(sideEffect)
            assertEquals(expectedResult, sideEffect.result)
        }
    }

    @Test
    fun `when failing to rename a list should update ui state with error`() = runTest {
        // Arrange
        val customListId = CustomListId("id2")
        val customListName = CustomListName.fromString("list2")
        val expectedError = RenameError(NameAlreadyExists(customListName))
        val viewModel = createViewModel(customListId, customListName.value)
        coEvery { mockCustomListActionUseCase(any<CustomListAction.Rename>()) } returns
            expectedError.left()

        // Act, Assert
        viewModel.uiState.test {
            awaitItem() // Default state
            viewModel.updateCustomListName(customListName.value)
            assertEquals(expectedError, awaitItem().error)
        }
    }

    @Test
    fun `given error state when calling clear error then should update to state without error`() =
        runTest {
            // Arrange
            val customListId = CustomListId("id")
            val customListName = CustomListName.fromString("list")
            val expectedError = RenameError(NameAlreadyExists(customListName))
            val viewModel = createViewModel(customListId, customListName.value)
            coEvery { mockCustomListActionUseCase(any<CustomListAction.Rename>()) } returns
                expectedError.left()

            // Act, Assert
            viewModel.uiState.test {
                awaitItem() // Default state
                viewModel.updateCustomListName(customListName.value)
                assertEquals(expectedError, awaitItem().error) // Showing error
            }
        }

    private fun createViewModel(customListId: CustomListId, initialName: String) =
        EditCustomListNameDialogViewModel(
            navArgs =
                EditCustomListNameNavKey(
                    customListId = customListId,
                    initialName = CustomListName.fromString(initialName),
                ),
            customListActionUseCase = mockCustomListActionUseCase,
        )
}
