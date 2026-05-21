package com.warrenbrowse.vpn.feature.serveripoverride.impl

import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import de.mannodermaus.junit5.compose.ComposeContext
import io.mockk.MockKAnnotations
import io.mockk.mockk
import io.mockk.verify
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.toLc
import com.warrenbrowse.vpn.lib.ui.tag.SERVER_IP_OVERRIDE_IMPORT_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.SERVER_IP_OVERRIDE_INFO_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.SERVER_IP_OVERRIDE_MORE_VERT_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.SERVER_IP_OVERRIDE_RESET_OVERRIDES_TEST_TAG
import com.warrenbrowse.vpn.screen.test.createEdgeToEdgeComposeExtension
import com.warrenbrowse.vpn.screen.test.setContentWithTheme
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.RegisterExtension

@ExperimentalTestApi
class ServerIpOverridesScreenTest {
    @JvmField @RegisterExtension val composeExtension = createEdgeToEdgeComposeExtension()

    @BeforeEach
    fun setup() {
        MockKAnnotations.init(this)
    }

    private fun ComposeContext.initScreen(
        state: Lc<Boolean, ServerIpOverridesUiState>,
        onBackClick: () -> Unit = {},
        onInfoClick: () -> Unit = {},
        onResetOverridesClick: () -> Unit = {},
        onImportClick: (Boolean) -> Unit = {},
    ) {
        setContentWithTheme {
            ServerIpOverridesScreen(
                state = state,
                onBackClick = onBackClick,
                onInfoClick = onInfoClick,
                onResetOverridesClick = onResetOverridesClick,
                onImportClick = onImportClick,
            )
        }
    }

    @Test
    fun ensureOverridesInactiveIsDisplayed() = composeExtension.use {
        // Arrange
        initScreen(state = ServerIpOverridesUiState(false).toLc())

        // Assert
        onNodeWithText("Overrides inactive").assertExists()
    }

    @Test
    fun ensureOverridesActiveIsDisplayed() = composeExtension.use {
        // Arrange
        initScreen(state = ServerIpOverridesUiState(true).toLc())

        // Assert
        onNodeWithText("Overrides active").assertExists()
    }

    @Test
    fun ensureOverridesImportButtonCanBeClicked() = composeExtension.use {
        // Arrange
        initScreen(state = ServerIpOverridesUiState(true).toLc())

        // Act
        onNodeWithTag(testTag = SERVER_IP_OVERRIDE_IMPORT_TEST_TAG).performClick()
    }

    @Test
    fun ensureInfoClickWorks() = composeExtension.use {
        // Arrange
        val clickHandler: () -> Unit = mockk(relaxed = true)
        initScreen(state = ServerIpOverridesUiState(false).toLc(), onInfoClick = clickHandler)

        // Act
        onNodeWithTag(SERVER_IP_OVERRIDE_INFO_TEST_TAG).performClick()

        // Assert
        verify { clickHandler() }
    }

    @Test
    fun ensureResetClickWorks() = composeExtension.use {
        // Arrange
        val clickHandler: () -> Unit = mockk(relaxed = true)
        initScreen(
            state = ServerIpOverridesUiState(true).toLc(),
            onResetOverridesClick = clickHandler,
        )

        // Act
        onNodeWithTag(SERVER_IP_OVERRIDE_MORE_VERT_TEST_TAG).performClick()
        onNodeWithTag(SERVER_IP_OVERRIDE_RESET_OVERRIDES_TEST_TAG).performClick()

        // Assert
        verify { clickHandler() }
    }
}
