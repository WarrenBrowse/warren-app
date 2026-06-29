package com.warrenbrowse.vpn.feature.settings.impl

import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.onNodeWithText
import de.mannodermaus.junit5.compose.ComposeContext
import io.mockk.MockKAnnotations
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.toLc
import com.warrenbrowse.vpn.screen.test.createEdgeToEdgeComposeExtension
import com.warrenbrowse.vpn.screen.test.setContentWithTheme
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.RegisterExtension

@OptIn(ExperimentalTestApi::class)
class SettingsScreenTest {
    @JvmField @RegisterExtension val composeExtension = createEdgeToEdgeComposeExtension()

    @BeforeEach
    fun setup() {
        MockKAnnotations.init(this)
    }

    private fun ComposeContext.initScreen(
        state: Lc<Unit, SettingsUiState>,
        onSplitTunnelingCellClick: () -> Unit = {},
        onAppInfoClick: () -> Unit = {},
        onReportProblemCellClick: () -> Unit = {},
        onApiAccessClick: () -> Unit = {},
        onMultihopClick: () -> Unit = {},
        onDaitaClick: () -> Unit = {},
        onBackClick: () -> Unit = {},
        onNotificationSettingsCellClick: () -> Unit = {},
    ) {
        setContentWithTheme {
            SettingsScreen(
                state = state,
                onSplitTunnelingCellClick = onSplitTunnelingCellClick,
                onAppInfoClick = onAppInfoClick,
                onReportProblemCellClick = onReportProblemCellClick,
                onApiAccessClick = onApiAccessClick,
                onMultihopClick = onMultihopClick,
                onDaitaClick = onDaitaClick,
                onBackClick = onBackClick,
                onNotificationSettingsCellClick = onNotificationSettingsCellClick,
            )
        }
    }

    @Test
    @OptIn(ExperimentalMaterial3Api::class)
    fun testLoggedInState() = composeExtension.use {
        // Arrange
        initScreen(
            state =
                SettingsUiState(
                        appVersion = "",
                        isLoggedIn = true,
                        isSupportedVersion = true,
                        isPlayBuild = false,
                        isDaitaEnabled = false,
                    )
                    .toLc()
        )
        // Assert
        onNodeWithText("VPN settings").assertExists()
        onNodeWithText("Split tunneling").assertExists()
        onNodeWithText("App info").assertExists()
        onNodeWithText("API access").assertExists()
    }

    @Test
    @OptIn(ExperimentalMaterial3Api::class)
    fun testLoggedOutState() = composeExtension.use {
        // Arrange
        initScreen(
            state =
                SettingsUiState(
                        appVersion = "",
                        isLoggedIn = false,
                        isSupportedVersion = true,
                        isPlayBuild = false,
                        isDaitaEnabled = false,
                    )
                    .toLc()
        )
        // Assert
        onNodeWithText("VPN settings").assertDoesNotExist()
        onNodeWithText("Split tunneling").assertDoesNotExist()
        onNodeWithText("App info").assertExists()
        onNodeWithText("API access").assertExists()
    }
}
