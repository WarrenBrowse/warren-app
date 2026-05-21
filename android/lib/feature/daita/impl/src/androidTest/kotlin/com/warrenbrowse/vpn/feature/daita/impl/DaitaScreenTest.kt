package com.warrenbrowse.vpn.feature.daita.impl

import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.onNodeWithTag
import de.mannodermaus.junit5.compose.ComposeContext
import io.mockk.MockKAnnotations
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.ui.tag.CIRCULAR_PROGRESS_INDICATOR_TEST_TAG
import com.warrenbrowse.vpn.screen.test.createEdgeToEdgeComposeExtension
import com.warrenbrowse.vpn.screen.test.setContentWithTheme
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.RegisterExtension

@OptIn(ExperimentalTestApi::class)
class DaitaScreenTest {
    @JvmField @RegisterExtension val composeExtension = createEdgeToEdgeComposeExtension()

    @BeforeEach
    fun setup() {
        MockKAnnotations.init(this)
    }

    private fun ComposeContext.initScreen(
        state: Lc<Boolean, DaitaUiState> = Lc.Loading(false),
        onDaitaEnabled: (enable: Boolean) -> Unit = {},
        onDirectOnlyClick: (enable: Boolean) -> Unit = {},
        onDirectOnlyInfoClick: () -> Unit = {},
        onBackClick: () -> Unit = {},
    ) {
        setContentWithTheme {
            DaitaScreen(
                state,
                onDaitaEnabled,
                onDirectOnlyClick,
                onDirectOnlyInfoClick,
                onBackClick,
            )
        }
    }

    @Test
    fun givenLoadingStateShouldShowLoadingSpinner() = composeExtension.use {
        // Arrange
        initScreen(state = Lc.Loading(true))

        // Assert
        onNodeWithTag(CIRCULAR_PROGRESS_INDICATOR_TEST_TAG).assertExists()
    }
}
