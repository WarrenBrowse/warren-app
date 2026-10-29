package com.warrenbrowse.vpn.feature.managedevices.impl

import android.graphics.Bitmap
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.isDialog
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.test.platform.app.InstrumentationRegistry
import com.warrenbrowse.vpn.feature.managedevices.impl.confirmation.ManageDevicesRemoveConfirmation
import com.warrenbrowse.vpn.lib.common.Lce
import com.warrenbrowse.vpn.lib.model.AccountDevice
import com.warrenbrowse.vpn.lib.model.AuthFailedError
import com.warrenbrowse.vpn.lib.model.DeviceRegistryError
import com.warrenbrowse.vpn.lib.model.ErrorState
import com.warrenbrowse.vpn.lib.model.ErrorStateCause
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.ui.component.AnimatedNotificationBanner
import com.warrenbrowse.vpn.screen.test.createEdgeToEdgeComposeExtension
import com.warrenbrowse.vpn.screen.test.setContentWithTheme
import de.mannodermaus.junit5.compose.ComposeContext
import java.io.File
import java.time.LocalDate
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.RegisterExtension

/**
 * The device list and the surfaces that open it (warren-core doc 115 section 7.1). Run with the
 * instrumentation argument `screenshots=true` to also write each surface as a PNG under the test
 * app's external files directory, for review.
 */
@OptIn(ExperimentalTestApi::class)
class ManageDevicesScreenTest {
    @JvmField @RegisterExtension val composeExtension = createEdgeToEdgeComposeExtension()

    private val today = LocalDate.of(2026, 10, 29)

    private fun item(
        id: String,
        name: String,
        platform: String,
        lastSeenOn: LocalDate,
        thisDevice: Boolean = false,
    ) =
        ManageDevicesItemUiState(
            AccountDevice(id, name, platform, "1.1.40", lastSeenOn, thisDevice),
            isLoading = false,
        )

    private val listed =
        Lce.Content(
            ManageDevicesUiState(
                devices =
                    listOf(
                        item("a", "brave otter", "android", today, thisDevice = true),
                        item("b", "quiet heron", "windows", today.minusDays(3)),
                        item("c", "swift lynx", "cli", today.minusDays(12)),
                    ),
                olderClientSeenOn = today.minusDays(1),
                today = today,
            )
        )

    @Test
    fun theListMarksThisDeviceAndOffersLogOutOnItDisconnectOnTheOthers() =
        composeExtension.use {
            var disconnected: String? = null
            var loggedOut: String? = null
            setContentWithTheme {
                ManageDevicesScreen(
                    state = listed,
                    onBackClick = {},
                    onTryAgainClicked = {},
                    onRename = {},
                    onDisconnect = { disconnected = it.id },
                    onLogOut = { loggedOut = it.id },
                )
            }

            onNodeWithText("This device").assertExists()
            onNodeWithText("Android · 1.1.40").assertExists()
            onNodeWithText("Active today").assertExists()
            onNodeWithText("An older version of Warren").assertExists()
            screenshot("device-list", onRoot())

            onNodeWithText("Log out").performClick()
            onNodeWithText("Disconnect").assertExists()
            assertEquals("a", loggedOut)
            assertEquals(null, disconnected)
        }

    @Test
    fun aListThatCouldNotLoadOffersARetry() =
        composeExtension.use {
            var retried = false
            setContentWithTheme {
                ManageDevicesScreen(
                    state = Lce.Error(DeviceRegistryError.Unavailable),
                    onBackClick = {},
                    onTryAgainClicked = { retried = true },
                    onRename = {},
                    onDisconnect = {},
                    onLogOut = {},
                )
            }

            onNodeWithText("Could not load your devices. Try again.").assertExists()
            screenshot("device-list-error", onRoot())
            onNodeWithText("Try again").performClick()
            assertEquals(true, retried)
        }

    @Test
    fun theDisconnectConfirmationNamesTheDevice() =
        composeExtension.use {
            var confirmed = false
            setContentWithTheme {
                ManageDevicesRemoveConfirmation(
                    name = "quiet heron",
                    onConfirm = { confirmed = true },
                    onBack = {},
                )
            }

            onNodeWithText("Disconnect quiet heron?").assertExists()
            screenshot("disconnect-confirmation", onNode(isDialog()))
            onNodeWithText("Disconnect").performClick()
            assertEquals(true, confirmed)
        }

    @Test
    fun theDeviceLimitBannerOpensTheDevices() =
        bannerOpensTheDevices(AuthFailedError.TooManyConnections, "banner-device-limit")

    @Test
    fun theNoSessionTokenBannerOpensTheDevices() =
        bannerOpensTheDevices(AuthFailedError.NoSessionToken, "banner-no-session-token")

    private fun bannerOpensTheDevices(error: AuthFailedError, name: String) =
        composeExtension.use {
            var opened = false
            setContentWithTheme {
                AnimatedNotificationBanner(
                    notification =
                        InAppNotification.TunnelStateError(
                            ErrorState(ErrorStateCause.AuthFailed(error), isBlocking = true)
                        ),
                    isPlayBuild = false,
                    openAppListing = {},
                    contentFocusRequester = FocusRequester(),
                    onClickShowAccount = {},
                    onClickShowChangelog = {},
                    onClickShowAndroid16UpgradeInfo = {},
                    onClickDismissChangelog = {},
                    onClickDismissAndroid16UpgradeWarning = {},
                    onClickDismissUpdateAvailable = {},
                    onClickDismissExitSwitched = {},
                    onClickDismissNotice = {},
                    onClickDismissAnnouncement = {},
                    onClickReEnableAfterStandDown = {},
                    onClickManageDevices = { opened = true },
                )
            }

            onNodeWithText("BLOCKING INTERNET").assertExists()
            screenshot(name, onRoot())
            onNodeWithText("Manage devices", substring = true).performClick()
            assertEquals(true, opened)
        }

    private fun ComposeContext.screenshot(name: String, node: SemanticsNodeInteraction) {
        val wanted = InstrumentationRegistry.getArguments().getString("screenshots") == "true"
        if (!wanted) return
        waitForIdle()
        val dir =
            File(
                InstrumentationRegistry.getInstrumentation().targetContext.getExternalFilesDir(null),
                "screenshots",
            )
        dir.mkdirs()
        File(dir, "$name.png").outputStream().use {
            node.captureToImage().asAndroidBitmap().compress(Bitmap.CompressFormat.PNG, 100, it)
        }
    }
}
