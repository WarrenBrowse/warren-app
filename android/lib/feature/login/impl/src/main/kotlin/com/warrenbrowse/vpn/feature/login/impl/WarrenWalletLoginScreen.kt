package com.warrenbrowse.vpn.feature.login.impl

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.core.text.HtmlCompat
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.warrenbrowse.vpn.common.compose.SecureScreenWhileInView
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnDisconnectInvoker
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithTopBar
import com.warrenbrowse.vpn.lib.ui.component.WarrenHelpLink
import com.warrenbrowse.vpn.lib.ui.component.dialog.NegativeConfirmationDialog
import com.warrenbrowse.vpn.lib.ui.component.toAnnotatedString
import com.warrenbrowse.vpn.lib.ui.component.wallet.BiometricPromptAuthorizer
import com.warrenbrowse.vpn.lib.ui.component.wallet.MnemonicInput
import com.warrenbrowse.vpn.lib.ui.component.wallet.countMnemonicWords
import com.warrenbrowse.vpn.lib.ui.designsystem.NegativeButton
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryTextButton
import com.warrenbrowse.vpn.lib.ui.designsystem.VariantButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import org.koin.androidx.compose.koinViewModel
import org.koin.compose.koinInject

/**
 * Wallet entry-point screen. Mirrors the desktop login UX: the Warren mark sits in the top bar
 * (like the desktop AppMainHeader), then the non-custodial explanation and two full-width choices.
 *
 * - `Create a new account` (positive/green CTA) emits `BackupGeneratedMnemonic` so the host
 *   NavController routes to `WarrenWalletBackupScreen`.
 * - `I already have an account` (primary/blue) opens an inline `MnemonicInput` for the 12-word
 *   phrase, then `WarrenWalletViewModel.importWallet`, which emits `WalletReady` on success.
 *
 * The ViewModel owns the repository interaction, the phrase being typed (so a rotation does not
 * wipe it) and one-shot event dispatch (Channel) so re-emission on config change does not
 * re-navigate.
 */
@Composable
fun WarrenWalletLoginScreen(
    onWalletCreated: (Mnemonic) -> Unit,
    onWalletReady: () -> Unit,
    modifier: Modifier = Modifier,
    onSettingsClicked: (() -> Unit)? = null,
    vm: WarrenWalletViewModel = koinViewModel(),
) {
    val state by vm.state.collectAsStateWithLifecycle()
    val busy by vm.busy.collectAsStateWithLifecycle()
    val importPhrase by vm.importPhrase.collectAsStateWithLifecycle()

    // Desktop LoginView `BlockMessage`: a user who logged out under lockdown, or
    // whose tunnel dropped while the kill switch holds, is offline and this
    // screen is the only one telling them why and how to release it.
    val connectionProxy = koinInject<ConnectionProxy>()
    val localSettings = koinInject<WarrenLocalSettingsRepository>()
    val disconnectInvoker = koinInject<WarrenQuinnDisconnectInvoker>()
    val tunnelState by
        connectionProxy.tunnelState.collectAsStateWithLifecycle(
            initialValue = TunnelState.Disconnected()
        )
    val lockdownMode by localSettings.lockdownMode.collectAsStateWithLifecycle()
    val blockNotice = loginBlockNotice(tunnelState, lockdownMode)

    // Used only when hardware-bound keystore auth is enabled: the encrypt step
    // at wallet create/import is gated by a CryptoObject prompt. With the flag
    // off the authorizer is ignored, so no prompt appears at creation.
    val activity = LocalContext.current as FragmentActivity
    val authorizer = remember(activity) { BiometricPromptAuthorizer(activity) }

    // Only the mode is saveable: the phrase itself lives in the ViewModel,
    // which survives a rotation without writing the secret to the
    // system-managed saved-state Bundle.
    var importMode by rememberSaveable { mutableStateOf(false) }
    var inlineError by remember { mutableStateOf<String?>(null) }
    var confirmOverwrite by remember { mutableStateOf(false) }

    val wrongWordCountError = stringResource(R.string.wallet_import_error_word_count)
    val invalidPhraseError = stringResource(R.string.wallet_import_error_invalid)
    val createFailedError = stringResource(R.string.wallet_create_error)

    LaunchedEffect(Unit) {
        vm.events.collect { event ->
            when (event) {
                is WarrenWalletEvent.BackupGeneratedMnemonic -> onWalletCreated(event.mnemonic)
                WarrenWalletEvent.WalletReady -> onWalletReady()
                is WarrenWalletEvent.Error ->
                    inlineError =
                        when (event.reason) {
                            WalletErrorReason.WrongWordCount -> wrongWordCountError
                            WalletErrorReason.InvalidPhrase -> invalidPhraseError
                            WalletErrorReason.CreateFailed -> createFailedError
                        }
            }
        }
    }

    val leaveImportMode = {
        importMode = false
        inlineError = null
    }
    // The in-page Back button and the system back must do the same thing: this
    // screen is normally the only back-stack entry, so an unhandled system back
    // quits the app in the middle of typing a recovery phrase.
    BackHandler(enabled = importMode) { leaveImportMode() }

    // Creating over an existing wallet silently replaces an identity whose
    // phrase may never have been written down, and which may already hold
    // credit. Desktop cannot reach this screen with a wallet present.
    val walletPresent = state !is WalletState.Absent
    val startCreate = {
        inlineError = null
        if (walletPresent) confirmOverwrite = true else vm.createWallet(authorizer)
    }

    ScaffoldWithTopBar(
        modifier = modifier,
        topBarColor = MaterialTheme.colorScheme.surface,
        onSettingsClicked = onSettingsClicked,
        onAccountClicked = null,
    ) { pv ->
        Column(
            modifier =
                Modifier.fillMaxSize()
                    .background(MaterialTheme.colorScheme.surface)
                    .padding(pv)
                    .verticalScroll(rememberScrollState())
                    .padding(
                        start = Dimens.sideMargin,
                        end = Dimens.sideMargin,
                        top = Dimens.screenTopMargin,
                        bottom = Dimens.screenBottomMargin,
                    ),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(Dimens.mediumPadding),
        ) {
            if (importMode) {
                // The phrase being entered is on screen; block screenshots and Recents.
                SecureScreenWhileInView()
                Text(
                    text = stringResource(R.string.wallet_import_title),
                    style = MaterialTheme.typography.headlineSmall,
                    color = MaterialTheme.colorScheme.onSurface,
                    textAlign = TextAlign.Center,
                )
                Text(
                    text = stringResource(R.string.wallet_login_import_prompt),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center,
                )

                MnemonicInput(
                    phrase = importPhrase,
                    onPhraseChange = { phrase ->
                        vm.setImportPhrase(phrase)
                        inlineError = null
                    },
                )
                InlineError(inlineError)

                Column(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(Dimens.buttonSpacing),
                ) {
                    val importWordCount = countMnemonicWords(importPhrase)
                    VariantButton(
                        modifier = Modifier.fillMaxWidth(),
                        onClick = { vm.importWallet(authorizer) },
                        text = stringResource(R.string.wallet_import_cta),
                        isEnabled = importWordCount == 12 || importWordCount == 24,
                        isLoading = busy,
                    )
                    PrimaryTextButton(
                        onClick = leaveImportMode,
                        text = stringResource(R.string.back),
                        isEnabled = !busy,
                    )
                }
            } else {
                if (blockNotice != null) {
                    BlockMessage(
                        notice = blockNotice,
                        onUnblock = {
                            // Disabling lockdown alone leaves the blackhole up
                            // until the adapter acts; the release is the
                            // disconnect, exactly as desktop's `unlock`.
                            if (blockNotice == LoginBlockNotice.LockdownMode) {
                                localSettings.setLockdownMode(false)
                            }
                            disconnectInvoker.disconnect()
                        },
                    )
                }
                Text(
                    text = stringResource(R.string.wallet_login_title),
                    style = MaterialTheme.typography.headlineSmall,
                    color = MaterialTheme.colorScheme.onSurface,
                    textAlign = TextAlign.Center,
                )
                Text(
                    text = stringResource(R.string.wallet_login_subtitle),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center,
                )
                InlineError(inlineError)

                Column(
                    modifier = Modifier.fillMaxWidth().padding(top = Dimens.mediumPadding),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(Dimens.buttonSpacing),
                ) {
                    VariantButton(
                        modifier = Modifier.fillMaxWidth(),
                        onClick = startCreate,
                        text = stringResource(R.string.wallet_create_cta),
                        isLoading = busy,
                    )
                    PrimaryButton(
                        modifier = Modifier.fillMaxWidth(),
                        onClick = {
                            inlineError = null
                            importMode = true
                        },
                        text = stringResource(R.string.wallet_import_entry_cta),
                        isEnabled = !busy,
                    )
                }
            }
        }
    }

    if (confirmOverwrite) {
        NegativeConfirmationDialog(
            message = stringResource(R.string.wallet_create_overwrite_confirm),
            confirmationText = stringResource(R.string.wallet_create_overwrite_action),
            onConfirm = {
                confirmOverwrite = false
                vm.createWallet(authorizer)
            },
            onBack = { confirmOverwrite = false },
        )
    }
}

/** Desktop `BlockMessage`: title, cause, and the one destructive action that releases traffic. */
@Composable
private fun BlockMessage(notice: LoginBlockNotice, onUnblock: () -> Unit) {
    val message =
        when (notice) {
            LoginBlockNotice.LockdownMode ->
                stringResource(
                    R.string.login_blocking_lockdown,
                    stringResource(R.string.tunnel_lockdown_mode_title),
                )
            LoginBlockNotice.KillSwitch -> stringResource(R.string.login_blocking_kill_switch)
        }
    Column(
        modifier = Modifier.fillMaxWidth(),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(Dimens.smallPadding),
    ) {
        Text(
            text = stringResource(R.string.blocking_internet),
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.error,
            textAlign = TextAlign.Center,
        )
        Text(
            text =
                HtmlCompat.fromHtml(message, HtmlCompat.FROM_HTML_MODE_COMPACT)
                    .toAnnotatedString(boldSpanStyle = SpanStyle(fontWeight = FontWeight.Bold)),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
        NegativeButton(
            modifier = Modifier.fillMaxWidth(),
            onClick = onUnblock,
            text =
                stringResource(
                    if (notice == LoginBlockNotice.LockdownMode) R.string.disable
                    else R.string.unblock
                ),
        )
    }
}

/** Error line plus the help-page link desktop shows under every login error. */
@Composable
private fun InlineError(message: String?) {
    message ?: return
    Text(
        text = message,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.error,
        textAlign = TextAlign.Center,
    )
    WarrenHelpLink()
}
