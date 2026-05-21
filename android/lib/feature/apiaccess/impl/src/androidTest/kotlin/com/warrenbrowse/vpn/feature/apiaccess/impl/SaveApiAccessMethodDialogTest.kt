package com.warrenbrowse.vpn.feature.apiaccess.impl

import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import de.mannodermaus.junit5.compose.ComposeContext
import io.mockk.mockk
import io.mockk.verify
import com.warrenbrowse.vpn.feature.apiaccess.impl.screen.save.SaveApiAccessMethodDialog
import com.warrenbrowse.vpn.feature.apiaccess.impl.screen.save.SaveApiAccessMethodUiState
import com.warrenbrowse.vpn.feature.apiaccess.impl.screen.save.TestApiAccessMethodState
import com.warrenbrowse.vpn.lib.ui.tag.SAVE_API_ACCESS_METHOD_CANCEL_BUTTON_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.SAVE_API_ACCESS_METHOD_LOADING_SPINNER_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.SAVE_API_ACCESS_METHOD_SAVE_BUTTON_TEST_TAG
import com.warrenbrowse.vpn.screen.test.createEdgeToEdgeComposeExtension
import com.warrenbrowse.vpn.screen.test.setContentWithTheme
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.RegisterExtension

@OptIn(ExperimentalTestApi::class)
class SaveApiAccessMethodDialogTest {
    @JvmField @RegisterExtension val composeExtension = createEdgeToEdgeComposeExtension()

    private fun ComposeContext.initDialog(
        state: SaveApiAccessMethodUiState = SaveApiAccessMethodUiState(),
        onCancel: () -> Unit = {},
        onSave: () -> Unit = {},
    ) {
        setContentWithTheme {
            SaveApiAccessMethodDialog(state = state, onCancel = onCancel, onSave = onSave)
        }
    }

    @Test
    fun whenTestingInProgressShouldShowSpinnerWithCancelButton() = composeExtension.use {
        // Arrange
        initDialog(
            state =
                SaveApiAccessMethodUiState(
                    testingState = TestApiAccessMethodState.Testing,
                    isSaving = false,
                )
        )

        // Assert
        onNodeWithTag(SAVE_API_ACCESS_METHOD_LOADING_SPINNER_TEST_TAG).assertExists()
        onNodeWithTag(SAVE_API_ACCESS_METHOD_CANCEL_BUTTON_TEST_TAG).assertExists()
    }

    @Test
    fun whenTestingFailedShouldShowSaveAndCancelButton() = composeExtension.use {
        // Arrange
        initDialog(
            state =
                SaveApiAccessMethodUiState(
                    testingState = TestApiAccessMethodState.Result.Failure,
                    isSaving = false,
                )
        )

        // Assert
        onNodeWithTag(SAVE_API_ACCESS_METHOD_SAVE_BUTTON_TEST_TAG).assertExists()
        onNodeWithTag(SAVE_API_ACCESS_METHOD_CANCEL_BUTTON_TEST_TAG).assertExists()
    }

    @Test
    fun whenTestingSuccessfulAndSavingShouldShowDisabledCancelButton() = composeExtension.use {
        // Arrange
        initDialog(
            state =
                SaveApiAccessMethodUiState(
                    testingState = TestApiAccessMethodState.Result.Successful,
                    isSaving = true,
                )
        )

        // Assert
        onNodeWithTag(SAVE_API_ACCESS_METHOD_CANCEL_BUTTON_TEST_TAG).assertExists()
        onNodeWithTag(SAVE_API_ACCESS_METHOD_CANCEL_BUTTON_TEST_TAG).assertIsNotEnabled()
    }

    @Test
    fun whenTestingInProgressAndClickingCancelShouldCallOnCancel() = composeExtension.use {
        // Arrange
        val onCancelClick: () -> Unit = mockk(relaxed = true)
        initDialog(
            state =
                SaveApiAccessMethodUiState(
                    testingState = TestApiAccessMethodState.Testing,
                    isSaving = false,
                ),
            onCancel = onCancelClick,
        )

        // Act
        onNodeWithTag(SAVE_API_ACCESS_METHOD_CANCEL_BUTTON_TEST_TAG).performClick()

        // Assert
        verify { onCancelClick() }
    }

    @Test
    fun whenTestingFailedAndClickingSaveShouldCallOnSave() = composeExtension.use {
        // Arrange
        val onSaveClick: () -> Unit = mockk(relaxed = true)
        initDialog(
            state =
                SaveApiAccessMethodUiState(
                    testingState = TestApiAccessMethodState.Result.Failure,
                    isSaving = false,
                ),
            onSave = onSaveClick,
        )

        // Act
        onNodeWithTag(SAVE_API_ACCESS_METHOD_SAVE_BUTTON_TEST_TAG).performClick()

        // Assert
        verify { onSaveClick() }
    }
}
