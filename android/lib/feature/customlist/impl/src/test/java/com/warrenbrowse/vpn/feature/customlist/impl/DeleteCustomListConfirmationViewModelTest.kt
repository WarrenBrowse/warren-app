package com.warrenbrowse.vpn.feature.customlist.impl

import app.cash.turbine.test
import arrow.core.right
import io.mockk.coEvery
import io.mockk.every
import io.mockk.mockk
import kotlin.test.assertIs
import kotlinx.coroutines.test.runTest
import com.warrenbrowse.vpn.feature.customlist.api.DeleteCustomListNavKey
import com.warrenbrowse.vpn.feature.customlist.impl.screen.delete.DeleteCustomListConfirmationSideEffect
import com.warrenbrowse.vpn.feature.customlist.impl.screen.delete.DeleteCustomListConfirmationViewModel
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.CustomListId
import com.warrenbrowse.vpn.lib.model.CustomListName
import com.warrenbrowse.vpn.lib.model.communication.CustomListAction
import com.warrenbrowse.vpn.lib.model.communication.CustomListActionResultData
import com.warrenbrowse.vpn.lib.model.communication.Deleted
import com.warrenbrowse.vpn.lib.usecase.customlists.CustomListActionUseCase
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith

@ExtendWith(TestCoroutineRule::class)
class DeleteCustomListConfirmationViewModelTest {
    private val mockCustomListActionUseCase: CustomListActionUseCase = mockk()

    @Test
    fun `when successfully deleting a list should emit return with result side effect`() = runTest {
        // Arrange
        val deleted: Deleted = mockk()
        val customListName = CustomListName.fromString("name")
        val undo: CustomListAction.Create = mockk()
        val expectedResult =
            CustomListActionResultData.Success.Deleted(customListName = customListName, undo = undo)
        every { deleted.name } returns customListName
        every { deleted.undo } returns undo
        val viewModel = createViewModel()
        coEvery { mockCustomListActionUseCase(any<CustomListAction.Delete>()) } returns
            deleted.right()

        // Act, Assert
        viewModel.uiSideEffect.test {
            viewModel.deleteCustomList()
            val sideEffect = awaitItem()
            assertIs<DeleteCustomListConfirmationSideEffect.ReturnWithResult>(sideEffect)
            assertEquals(expectedResult, sideEffect.result)
        }
    }

    private fun createViewModel() =
        DeleteCustomListConfirmationViewModel(
            navArgs =
                DeleteCustomListNavKey(
                    customListId = CustomListId("1"),
                    name = CustomListName.fromString("asdf"),
                ),
            customListActionUseCase = mockCustomListActionUseCase,
        )
}
