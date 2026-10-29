package com.warrenbrowse.vpn.feature.managedevices.impl

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.tooling.preview.PreviewParameter
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.compose.dropUnlessResumed
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.common.compose.CollectSideEffectWithLifecycle
import com.warrenbrowse.vpn.common.compose.dropUnlessResumed
import com.warrenbrowse.vpn.common.compose.showSnackbarImmediately
import com.warrenbrowse.vpn.core.LocalResultStore
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.login.api.WarrenKeysNavKey
import com.warrenbrowse.vpn.feature.login.api.WarrenWalletNavKey
import com.warrenbrowse.vpn.feature.managedevices.api.ManageDevicesRemoveConfirmationNavKey
import com.warrenbrowse.vpn.feature.managedevices.api.ManageDevicesRemoveConfirmationNavResult
import com.warrenbrowse.vpn.lib.common.Lce
import com.warrenbrowse.vpn.lib.model.AccountDevice
import com.warrenbrowse.vpn.lib.model.DEVICE_NAME_MAX_CHARS
import com.warrenbrowse.vpn.lib.model.DeviceRegistryError
import com.warrenbrowse.vpn.lib.model.isValidDeviceName
import com.warrenbrowse.vpn.lib.repository.MnemonicCache
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.component.dialog.InputDialog
import com.warrenbrowse.vpn.lib.ui.component.dialog.LogoutConfirmationDialog
import com.warrenbrowse.vpn.lib.ui.component.drawVerticalScrollbar
import com.warrenbrowse.vpn.lib.ui.component.listitem.DeviceListItem
import com.warrenbrowse.vpn.lib.ui.component.positionForIndex
import com.warrenbrowse.vpn.lib.ui.component.textfield.CustomTextField
import com.warrenbrowse.vpn.lib.ui.component.wallet.BiometricPromptAuthorizer
import com.warrenbrowse.vpn.lib.ui.designsystem.Hierarchy
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenCircularProgressIndicatorMedium
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenListItem
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.AlphaScrollbar
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import java.util.Locale
import kotlinx.coroutines.launch
import org.koin.androidx.compose.koinViewModel
import org.koin.compose.koinInject

private typealias StateLce = Lce<Unit, ManageDevicesUiState, DeviceRegistryError>

@Composable
@Preview("Content|Loading|Error")
private fun PreviewManageDevicesScreen(
    @PreviewParameter(ManageDevicesUiStatePreviewParameterProvider::class) state: StateLce
) {
    AppTheme {
        ManageDevicesScreen(
            state = state,
            onBackClick = {},
            onTryAgainClicked = {},
            onRename = {},
            onDisconnect = {},
            onLogOut = {},
        )
    }
}

@Composable
fun ManageDevices(navigator: Navigator) {
    val viewModel = koinViewModel<ManageDevicesViewModel>()
    val state by viewModel.uiState.collectAsStateWithLifecycle()

    LocalResultStore.current.consumeResult<ManageDevicesRemoveConfirmationNavResult> {
        viewModel.removeDevice(it.deviceId)
    }

    var renaming by rememberSaveable { mutableStateOf<Pair<String, String>?>(null) }
    var loggingOut by rememberSaveable { mutableStateOf<String?>(null) }

    val snackbarHostState = remember { SnackbarHostState() }
    val resources = LocalResources.current
    CollectSideEffectWithLifecycle(
        viewModel.uiSideEffect,
        minActiveState = Lifecycle.State.RESUMED,
    ) { sideEffect ->
        val message =
            when (sideEffect) {
                ManageDevicesSideEffect.FailedToDisconnect -> R.string.device_disconnect_failed
                ManageDevicesSideEffect.FailedToRename -> R.string.device_rename_failed
                ManageDevicesSideEffect.FailedToLogOut -> R.string.wallet_settings_erase_failed
                ManageDevicesSideEffect.LoggedOut -> {
                    navigator.navigate(WarrenWalletNavKey(), clearBackStack = true)
                    null
                }
            }
        message?.let {
            launch { snackbarHostState.showSnackbarImmediately(message = resources.getString(it)) }
        }
    }

    ManageDevicesScreen(
        state = state,
        snackbarHostState = snackbarHostState,
        onBackClick = dropUnlessResumed { navigator.goBack() },
        onTryAgainClicked = { viewModel.fetchDevices() },
        onRename = { renaming = it.id to it.displayedName },
        onDisconnect =
            dropUnlessResumed<AccountDevice> {
                navigator.navigate(ManageDevicesRemoveConfirmationNavKey(it.id, it.displayedName))
            },
        onLogOut = { loggingOut = it.id },
    )

    renaming?.let { (id, name) ->
        RenameDeviceDialog(
            initialName = name,
            onSave = {
                renaming = null
                viewModel.renameDevice(id, it)
            },
            onDismiss = { renaming = null },
        )
    }

    loggingOut?.let { id ->
        val revealPhrase = rememberRevealPhrase(navigator)
        LogoutConfirmationDialog(
            onLogOut = {
                loggingOut = null
                viewModel.logOutThisDevice(id)
            },
            onBackUpPhrase = {
                loggingOut = null
                revealPhrase()
            },
            onDismiss = { loggingOut = null },
        )
    }
}

/**
 * The logout dialog's escape hatch, as on the account screen: the phrase is unlocked behind the
 * biometric prompt and staged out of band for the keys screen, never in a NavKey.
 */
@Composable
private fun rememberRevealPhrase(navigator: Navigator): () -> Unit {
    val activity = LocalContext.current as FragmentActivity
    val wallet = koinInject<WalletRepository>()
    val scope = rememberCoroutineScope()
    val reason = stringResource(R.string.wallet_biometric_view_phrase_reason)
    return {
        scope.launch {
            // A cancelled prompt or an unreadable wallet leaves the user on the list.
            @Suppress("TooGenericExceptionCaught")
            try {
                MnemonicCache.put(wallet.unlock(BiometricPromptAuthorizer(activity), reason))
                navigator.navigate(WarrenKeysNavKey)
            } catch (e: Exception) {
                Logger.w { "wallet unlock for the backup failed (${e::class.simpleName})" }
            }
        }
    }
}

@Composable
fun ManageDevicesScreen(
    state: StateLce,
    snackbarHostState: SnackbarHostState = remember { SnackbarHostState() },
    onBackClick: () -> Unit,
    onTryAgainClicked: () -> Unit,
    onRename: (AccountDevice) -> Unit,
    onDisconnect: (AccountDevice) -> Unit,
    onLogOut: (AccountDevice) -> Unit,
) {
    ScaffoldWithSmallTopBar(
        appBarTitle = stringResource(id = R.string.devices_title),
        navigationIcon = { NavigateBackIconButton(onNavigateBack = onBackClick) },
        snackbarHostState = snackbarHostState,
    ) { modifier ->
        when (state) {
            is Lce.Content -> Content(modifier, state.value, onRename, onDisconnect, onLogOut)
            is Lce.Error -> Error(modifier, onTryAgainClicked)
            is Lce.Loading -> Loading(modifier)
        }
    }
}

@Composable
private fun Content(
    modifier: Modifier,
    state: ManageDevicesUiState,
    onRename: (AccountDevice) -> Unit,
    onDisconnect: (AccountDevice) -> Unit,
    onLogOut: (AccountDevice) -> Unit,
) {
    val scrollState = rememberScrollState()
    val locale = LocalConfiguration.current.locales[0]
    Column(
        modifier
            .drawVerticalScrollbar(
                state = scrollState,
                color = MaterialTheme.colorScheme.onSurface.copy(alpha = AlphaScrollbar),
            )
            .verticalScroll(state = scrollState)
            .padding(horizontal = Dimens.sideMarginNew)
    ) {
        Text(
            text = stringResource(id = R.string.devices_description),
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(Dimens.verticalSpace))
        val rows = state.devices.size + if (state.olderClientSeenOn != null) 1 else 0
        state.devices.forEachIndexed { index, (device, isLoading) ->
            DeviceListItem(
                position = positionOf(index, rows),
                name = device.displayedName,
                thisDeviceBadge =
                    if (device.thisDevice) stringResource(R.string.device_this_device) else null,
                platformLine = "${platformLabel(device.platform)} · ${device.appVersion}",
                activity = device.lastSeenOn?.let { activityLabel(it, state.today, locale) },
                actionLabel =
                    stringResource(
                        if (device.thisDevice) R.string.wallet_settings_erase
                        else R.string.device_disconnect
                    ),
                isLoading = isLoading,
                onRename = { onRename(device) },
                onAction = { if (device.thisDevice) onLogOut(device) else onDisconnect(device) },
            )
            if (index != rows - 1) Spacer(Modifier.height(Dimens.listItemDivider))
        }
        state.olderClientSeenOn?.let { OlderClientItem(positionOf(rows - 1, rows), it, locale) }
    }
}

private fun positionOf(index: Int, count: Int): Position =
    List(count) { it }.positionForIndex(index)

/**
 * A device too old to register: it cannot be named or disconnected, only pointed at, since a
 * registered device already took its place.
 */
@Composable
private fun OlderClientItem(position: Position, day: LocalDate, locale: Locale) {
    WarrenListItem(
        hierarchy = Hierarchy.Parent,
        position = position,
        content = {
            Column(
                modifier = Modifier.padding(vertical = Dimens.smallPadding),
                verticalArrangement = Arrangement.spacedBy(Dimens.tinyPadding),
            ) {
                Text(
                    text = stringResource(R.string.device_older_client_title),
                    style = MaterialTheme.typography.titleSmall,
                )
                Text(
                    text = stringResource(R.string.device_older_client_body),
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Text(
                    text = stringResource(R.string.device_last_seen, shortDate(day, locale)),
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        },
    )
}

/** Product names stay as they are; the client kinds are words. */
@Composable
private fun platformLabel(platform: String): String =
    when (platform) {
        "android" -> "Android"
        "ios" -> "iOS"
        "macos" -> "macOS"
        "windows" -> "Windows"
        "linux" -> "Linux"
        "cli" -> stringResource(R.string.device_platform_command_line)
        "browser" -> stringResource(R.string.device_platform_browser_extension)
        else -> stringResource(R.string.device_platform_other)
    }

@Composable
private fun activityLabel(lastSeenOn: LocalDate, today: LocalDate, locale: Locale): String =
    if (lastSeenOn == today) {
        stringResource(R.string.device_active_today)
    } else {
        stringResource(R.string.device_last_active, shortDate(lastSeenOn, locale))
    }

private fun shortDate(day: LocalDate, locale: Locale): String =
    DateTimeFormatter.ofLocalizedDate(FormatStyle.SHORT).withLocale(locale).format(day)

@Composable
private fun Error(modifier: Modifier, tryAgain: () -> Unit) {
    Column(modifier, verticalArrangement = Arrangement.Center) {
        Text(
            text = stringResource(id = R.string.devices_load_failed),
            modifier = Modifier.padding(Dimens.smallPadding).align(Alignment.CenterHorizontally),
        )
        PrimaryButton(
            onClick = tryAgain,
            text = stringResource(id = R.string.devices_try_again),
            modifier =
                Modifier.padding(
                    top = Dimens.buttonSpacing,
                    start = Dimens.sideMarginNew,
                    end = Dimens.sideMarginNew,
                ),
        )
    }
}

@Composable
private fun Loading(modifier: Modifier) {
    Box(modifier, contentAlignment = Alignment.Center) {
        WarrenCircularProgressIndicatorMedium(modifier = Modifier.padding(Dimens.smallPadding))
    }
}

@Composable
private fun RenameDeviceDialog(
    initialName: String,
    onSave: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    var name by rememberSaveable { mutableStateOf(initialName) }
    val valid = isValidDeviceName(name)
    InputDialog(
        title = stringResource(R.string.device_rename_title),
        confirmButtonText = stringResource(R.string.device_rename_save),
        onBack = onDismiss,
        onConfirm = { if (valid) onSave(name) },
        input = {
            CustomTextField(
                value = name,
                keyboardType = KeyboardType.Text,
                onValueChanged = { name = it },
                onSubmit = { if (valid) onSave(it) },
                maxCharLength = DEVICE_NAME_MAX_CHARS,
                isValidValue = valid,
                isDigitsOnlyAllowed = false,
            )
        },
    )
}
