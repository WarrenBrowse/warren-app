package com.warrenbrowse.vpn.feature.account.impl

import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.ui.test.ExperimentalTestApi
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
import com.warrenbrowse.vpn.lib.model.AccountNumber
import com.warrenbrowse.vpn.screen.test.createEdgeToEdgeComposeExtension
import com.warrenbrowse.vpn.screen.test.setContentWithTheme
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.RegisterExtension
import org.koin.core.context.loadKoinModules
import org.koin.core.module.dsl.viewModel
import org.koin.dsl.module

@ExperimentalTestApi
@OptIn(ExperimentalMaterial3Api::class)
class AccountScreenTest {
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
        state: AccountUiState? = null,
        onCopyAccountNumber: (String) -> Unit = {},
        onLogoutClick: () -> Unit = {},
        onPlayPaymentInfoClick: () -> Unit = {},
        onBackClick: () -> Unit = {},
        onManageDevicesClick: () -> Unit = {},
        navigateToDeleteAccount: () -> Unit = {},
        navigateToAddTime: () -> Unit = {},
    ) {
        setContentWithTheme {
            AccountScreen(
                state = state,
                onCopyAccountNumber = onCopyAccountNumber,
                onManageDevicesClick = onManageDevicesClick,
                onLogoutClick = onLogoutClick,
                onPlayPaymentInfoClick = onPlayPaymentInfoClick,
                onBackClick = onBackClick,
                navigateToDeleteAccount = navigateToDeleteAccount,
                navigateToAddTime = navigateToAddTime,
            )
        }
    }

    @Test
    fun testDefaultState() = composeExtension.use {
        // Arrange
        initScreen(
            state =
                AccountUiState(
                    deviceName = DUMMY_DEVICE_NAME,
                    accountNumber = DUMMY_ACCOUNT_NUMBER,
                    accountExpiry = null,
                    showLogoutLoading = false,
                    verificationPending = false,
                )
        )

        // Assert
        onNodeWithText("Log out").assertExists()
    }

    @Test
    fun testLogoutClick() = composeExtension.use {
        // Arrange
        val mockedClickHandler: () -> Unit = mockk(relaxed = true)
        initScreen(
            state =
                AccountUiState(
                    deviceName = DUMMY_DEVICE_NAME,
                    accountNumber = DUMMY_ACCOUNT_NUMBER,
                    accountExpiry = null,
                    showLogoutLoading = false,
                    verificationPending = false,
                ),
            onLogoutClick = mockedClickHandler,
        )

        // Act
        onNodeWithText("Log out").performClick()

        // Assert
        verify { mockedClickHandler.invoke() }
    }

    @Test
    fun testShowVerificationInProgress() = composeExtension.use {
        // Arrange
        initScreen(
            state =
                AccountUiState(
                    deviceName = DUMMY_DEVICE_NAME,
                    accountNumber = DUMMY_ACCOUNT_NUMBER,
                    accountExpiry = null,
                    showLogoutLoading = false,
                    verificationPending = true,
                )
        )

        // Assert
        onNodeWithText("Google Play payment pending").assertExists()
    }

    companion object {
        private const val DUMMY_DEVICE_NAME = "fake_name"
        private val DUMMY_ACCOUNT_NUMBER = AccountNumber("1234123412341234")
    }
}
