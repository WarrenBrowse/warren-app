package com.warrenbrowse.vpn.feature.serveripoverride.impl

import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import de.mannodermaus.junit5.compose.ComposeContext
import io.mockk.MockKAnnotations
import io.mockk.mockk
import io.mockk.verify
import com.warrenbrowse.vpn.feature.serveripoverride.impl.reset.ResetServerIpOverridesConfirmationDialog
import com.warrenbrowse.vpn.lib.ui.tag.RESET_SERVER_IP_OVERRIDE_CANCEL_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.RESET_SERVER_IP_OVERRIDE_RESET_TEST_TAG
import com.warrenbrowse.vpn.screen.test.createEdgeToEdgeComposeExtension
import com.warrenbrowse.vpn.screen.test.setContentWithTheme
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.RegisterExtension

class ResetServerIPOverridesConfirmationDialogTest {
    @OptIn(ExperimentalTestApi::class)
    @JvmField
    @RegisterExtension
    val composeExtension = createEdgeToEdgeComposeExtension()

    @BeforeEach
    fun setup() {
        MockKAnnotations.init(this)
    }

    private fun ComposeContext.initDialog(
        onClearAllOverrides: () -> Unit = {},
        onNavigateBack: () -> Unit = {},
    ) {
        setContentWithTheme {
            ResetServerIpOverridesConfirmationDialog(
                onClearAllOverrides = onClearAllOverrides,
                onNavigateBack = onNavigateBack,
            )
        }
    }

    @Test
    fun ensureCancelClickWorks() = composeExtension.use {
        val clickHandler: () -> Unit = mockk(relaxed = true)

        // Arrange
        initDialog(onNavigateBack = clickHandler)

        // Act
        onNodeWithTag(RESET_SERVER_IP_OVERRIDE_CANCEL_TEST_TAG).performClick()

        // Assert
        verify { clickHandler() }
    }

    @Test
    fun ensureResetClickWorks() = composeExtension.use {
        val clickHandler: () -> Unit = mockk(relaxed = true)

        // Arrange
        initDialog(onClearAllOverrides = clickHandler)

        // Act
        onNodeWithTag(RESET_SERVER_IP_OVERRIDE_RESET_TEST_TAG).performClick()

        // Assert
        verify { clickHandler() }
    }
}
