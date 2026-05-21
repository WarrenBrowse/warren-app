package com.warrenbrowse.vpn.feature.home.impl.welcome

import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import de.mannodermaus.junit5.compose.ComposeContext
import io.mockk.MockKAnnotations
import io.mockk.every
import io.mockk.mockk
import io.mockk.verify
import kotlinx.coroutines.flow.MutableStateFlow
import com.warrenbrowse.vpn.feature.addtime.impl.AddTimeUiState
import com.warrenbrowse.vpn.feature.addtime.impl.AddTimeViewModel
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.common.toLc
import com.warrenbrowse.vpn.lib.model.AccountNumber
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.ui.tag.PLAY_PAYMENT_INFO_ICON_TEST_TAG
import com.warrenbrowse.vpn.screen.test.createEdgeToEdgeComposeExtension
import com.warrenbrowse.vpn.screen.test.setContentWithTheme
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.RegisterExtension
import org.koin.core.context.loadKoinModules
import org.koin.core.module.dsl.viewModel
import org.koin.dsl.module

@OptIn(ExperimentalTestApi::class)
class WelcomeScreenTest {
    @JvmField @RegisterExtension val composeExtension = createEdgeToEdgeComposeExtension()

    private val addTimeViewModel: AddTimeViewModel = mockk(relaxed = true)

    @BeforeEach
    fun setup() {
        MockKAnnotations.init(this)
        loadKoinModules(module { viewModel { addTimeViewModel } })
        every { addTimeViewModel.uiState } returns
            MutableStateFlow<Lc<Unit, AddTimeUiState>>(Lc.Loading(Unit))
    }

    private fun ComposeContext.initScreen(
        state: Lc<Unit, WelcomeUiState> = Lc.Loading(Unit),
        onSettingsClick: () -> Unit = {},
        onAccountClick: () -> Unit = {},
        onDisconnectClick: () -> Unit = {},
        navigateToDeviceInfoDialog: () -> Unit = {},
        onPlayPaymentInfoClick: () -> Unit = {},
        onAddMoreTimeClick: () -> Unit = {},
    ) {
        setContentWithTheme {
            WelcomeScreen(
                state = state,
                onSettingsClick = onSettingsClick,
                onAccountClick = onAccountClick,
                navigateToDeviceInfoDialog = navigateToDeviceInfoDialog,
                onDisconnectClick = onDisconnectClick,
                onPlayPaymentInfoClick = onPlayPaymentInfoClick,
                onAddMoreTimeClick = onAddMoreTimeClick,
            )
        }
    }

    @Test
    fun testDefaultState() = composeExtension.use {
        // Arrange
        initScreen()

        // Assert
        onNodeWithText("Congrats!").assertExists()
        onNodeWithText("Here’s your account number. Save it!").assertExists()
    }

    @Test
    fun testDisableSitePayment() = composeExtension.use {
        // Arrange
        initScreen()

        // Assert
        onNodeWithText("Either buy credit on our website or redeem a voucher.", substring = true)
            .assertDoesNotExist()
    }

    @Test
    fun testShowAccountNumber() = composeExtension.use {
        // Arrange
        val rawAccountNumber = AccountNumber("1111222233334444")
        val expectedAccountNumber = "1111 2222 3333 4444"
        initScreen(
            state =
                WelcomeUiState(
                        tunnelState = TunnelState.Disconnected(),
                        accountNumber = rawAccountNumber,
                        deviceName = null,
                        showSitePayment = false,
                        verificationPending = false,
                    )
                    .toLc()
        )

        // Assert
        onNodeWithText(expectedAccountNumber).assertExists()
    }

    @Test
    fun testShowPendingPaymentInfoDialog() = composeExtension.use {
        // Arrange
        val mockShowPendingInfo = mockk<() -> Unit>(relaxed = true)
        initScreen(
            state =
                WelcomeUiState(
                        tunnelState = TunnelState.Disconnected(),
                        accountNumber = null,
                        deviceName = null,
                        showSitePayment = false,
                        verificationPending = true,
                    )
                    .toLc(),
            onPlayPaymentInfoClick = mockShowPendingInfo,
        )

        // Act
        onNodeWithTag(PLAY_PAYMENT_INFO_ICON_TEST_TAG).performClick()

        // Assert
        verify(exactly = 1) { mockShowPendingInfo() }
    }

    @Test
    fun testShowVerificationInProgress() = composeExtension.use {
        // Arrange
        initScreen(
            state =
                WelcomeUiState(
                        tunnelState = TunnelState.Disconnected(),
                        accountNumber = null,
                        deviceName = null,
                        showSitePayment = false,
                        verificationPending = true,
                    )
                    .toLc()
        )

        // Assert
        onNodeWithText("Google Play payment pending").assertExists()
    }

    @Test
    fun testOnDisconnectClick() = composeExtension.use {
        // Arrange
        val clickHandler: () -> Unit = mockk(relaxed = true)
        val tunnelState: TunnelState = mockk(relaxed = true)
        every { tunnelState.isSecured() } returns true
        initScreen(
            state =
                WelcomeUiState(
                        tunnelState = tunnelState,
                        accountNumber = null,
                        deviceName = null,
                        showSitePayment = false,
                        verificationPending = false,
                    )
                    .toLc(),
            onDisconnectClick = clickHandler,
        )

        // Act
        onNodeWithText("Disconnect").performClick()

        // Assert
        verify { clickHandler() }
    }
}
