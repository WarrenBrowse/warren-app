package com.warrenbrowse.vpn.feature.splittunneling.impl

import android.graphics.drawable.Drawable
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import de.mannodermaus.junit5.compose.ComposeContext
import io.mockk.MockKAnnotations
import io.mockk.mockk
import io.mockk.unmockkAll
import io.mockk.verify
import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.AppData
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.toLc
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.model.SplitTunnelMode
import com.warrenbrowse.vpn.screen.test.createEdgeToEdgeComposeExtension
import com.warrenbrowse.vpn.screen.test.setContentWithTheme
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.RegisterExtension

@OptIn(ExperimentalTestApi::class)
class SplitTunnelingScreenTest {
    @JvmField @RegisterExtension val composeExtension = createEdgeToEdgeComposeExtension()

    @BeforeEach
    fun setup() {
        MockKAnnotations.init(this)
    }

    @AfterEach
    fun tearDown() {
        unmockkAll()
    }

    private fun ComposeContext.initScreen(
        state: Lc<Loading, SplitTunnelingUiState>,
        onShowSystemAppsClick: (show: Boolean) -> Unit = {},
        onAddAppClick: (packageName: PackageName) -> Unit = {},
        onRemoveAppClick: (packageName: PackageName) -> Unit = {},
        onBackClick: () -> Unit = {},
        onResolveIcon: (PackageName) -> Drawable? = { null },
        navigateToSearch: () -> Unit = {},
    ) {
        setContentWithTheme {
            SplitTunnelingScreen(
                state = state,
                onSelectTab = {},
                onSplitModeSwitch = {},
                onConfirmModeChange = {},
                onCancelModeChange = {},
                onShowSystemAppsClick = onShowSystemAppsClick,
                onAddAppClick = onAddAppClick,
                onRemoveAppClick = onRemoveAppClick,
                onBackClick = onBackClick,
                onResolveIcon = onResolveIcon,
                navigateToSearch = navigateToSearch,
            )
        }
    }

    @Test
    fun testLoadingState() = composeExtension.use {
        // Arrange
        initScreen(state = Lc.Loading(Loading()))

        // Assert
        onNodeWithText(TITLE).assertExists()
        onNodeWithText(DESCRIPTION, substring = true).assertExists()
        onNodeWithText(EXCLUDED_APPLICATIONS).assertDoesNotExist()
        onNodeWithText(SHOW_SYSTEM_APPS).assertDoesNotExist()
        onNodeWithText(ALL_APPLICATIONS).assertDoesNotExist()
    }

    @Test
    fun testListDisplayed() = composeExtension.use {
        // Arrange
        val excludedApp =
            AppData(packageName = EXCLUDED_APP_PACKAGE_NAME, iconRes = 0, name = EXCLUDED_APP_NAME)
        val includedApp =
            AppData(packageName = INCLUDED_APP_PACKAGE_NAME, iconRes = 0, name = INCLUDED_APP_NAME)
        initScreen(
            state =
                SplitTunnelingUiState(
                        splitMode = SplitTunnelMode.Exclude,
                        selectedApps = listOf(excludedApp),
                        otherApps = listOf(includedApp),
                        showSystemApps = false,
                    )
                    .toLc()
        )

        // Assert
        onNodeWithText(TITLE).assertExists()
        onNodeWithText(DESCRIPTION, substring = true).assertExists()
        onNodeWithText(EXCLUDED_APPLICATIONS).assertExists()
        onNodeWithText(EXCLUDED_APP_NAME).assertExists()
        onNodeWithText(SHOW_SYSTEM_APPS).assertExists()
        onNodeWithText(ALL_APPLICATIONS).assertExists()
        onNodeWithText(INCLUDED_APP_NAME).assertExists()
    }

    @Test
    fun testNoExcludedApps() = composeExtension.use {
        // Arrange
        val includedApp =
            AppData(packageName = INCLUDED_APP_PACKAGE_NAME, iconRes = 0, name = INCLUDED_APP_NAME)
        initScreen(
            state =
                SplitTunnelingUiState(
                        splitMode = SplitTunnelMode.Exclude,
                        selectedApps = emptyList(),
                        otherApps = listOf(includedApp),
                        showSystemApps = false,
                    )
                    .toLc()
        )

        // Assert
        onNodeWithText(TITLE).assertExists()
        onNodeWithText(DESCRIPTION, substring = true).assertExists()
        onNodeWithText(EXCLUDED_APPLICATIONS).assertDoesNotExist()
        onNodeWithText(EXCLUDED_APP_NAME).assertDoesNotExist()
        onNodeWithText(SHOW_SYSTEM_APPS).assertExists()
        onNodeWithText(ALL_APPLICATIONS).assertExists()
        onNodeWithText(INCLUDED_APP_NAME).assertExists()
    }

    @Test
    fun testClickIncludedItem() = composeExtension.use {
        // Arrange
        val excludedApp =
            AppData(packageName = EXCLUDED_APP_PACKAGE_NAME, iconRes = 0, name = EXCLUDED_APP_NAME)
        val includedApp =
            AppData(packageName = INCLUDED_APP_PACKAGE_NAME, iconRes = 0, name = INCLUDED_APP_NAME)
        val mockedClickHandler: (PackageName) -> Unit = mockk(relaxed = true)
        initScreen(
            state =
                SplitTunnelingUiState(
                        splitMode = SplitTunnelMode.Exclude,
                        selectedApps = listOf(excludedApp),
                        otherApps = listOf(includedApp),
                        showSystemApps = false,
                    )
                    .toLc(),
            onAddAppClick = mockedClickHandler,
        )

        // Act
        onNodeWithText(INCLUDED_APP_NAME).performClick()

        // Assert
        verify { mockedClickHandler.invoke(INCLUDED_APP_PACKAGE_NAME) }
    }

    @Test
    fun testClickExcludedItem() = composeExtension.use {
        // Arrange
        val excludedApp =
            AppData(packageName = EXCLUDED_APP_PACKAGE_NAME, iconRes = 0, name = EXCLUDED_APP_NAME)
        val includedApp =
            AppData(packageName = INCLUDED_APP_PACKAGE_NAME, iconRes = 0, name = INCLUDED_APP_NAME)
        val mockedClickHandler: (PackageName) -> Unit = mockk(relaxed = true)
        initScreen(
            state =
                SplitTunnelingUiState(
                        splitMode = SplitTunnelMode.Exclude,
                        selectedApps = listOf(excludedApp),
                        otherApps = listOf(includedApp),
                        showSystemApps = false,
                    )
                    .toLc(),
            onRemoveAppClick = mockedClickHandler,
        )

        // Act
        onNodeWithText(EXCLUDED_APP_NAME).performClick()

        // Assert
        verify { mockedClickHandler.invoke(EXCLUDED_APP_PACKAGE_NAME) }
    }

    @Test
    fun testClickShowSystemApps() = composeExtension.use {
        // Arrange
        val excludedApp =
            AppData(packageName = EXCLUDED_APP_PACKAGE_NAME, iconRes = 0, name = EXCLUDED_APP_NAME)
        val includedApp =
            AppData(packageName = INCLUDED_APP_PACKAGE_NAME, iconRes = 0, name = INCLUDED_APP_NAME)
        val mockedClickHandler: (Boolean) -> Unit = mockk(relaxed = true)
        initScreen(
            state =
                SplitTunnelingUiState(
                        splitMode = SplitTunnelMode.Exclude,
                        selectedApps = listOf(excludedApp),
                        otherApps = listOf(includedApp),
                        showSystemApps = false,
                    )
                    .toLc(),
            onShowSystemAppsClick = mockedClickHandler,
        )

        // Act
        onNodeWithText(SHOW_SYSTEM_APPS).performClick()

        // Assert
        verify { mockedClickHandler.invoke(true) }
    }

    companion object {
        private val EXCLUDED_APP_PACKAGE_NAME = PackageName("excluded-pkg")
        private const val EXCLUDED_APP_NAME = "Excluded Name"
        private val INCLUDED_APP_PACKAGE_NAME = PackageName("included-pkg")
        private const val INCLUDED_APP_NAME = "Included Name"
        private const val TITLE = "App routing"
        private const val DESCRIPTION = "Choose how each app connects."
        private const val EXCLUDED_APPLICATIONS = "Excluded applications"
        private const val SHOW_SYSTEM_APPS = "Show system apps"
        private const val ALL_APPLICATIONS = "All applications"
    }
}
