package com.warrenbrowse.vpn.feature.login.impl

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.text.input.KeyboardType
import com.warrenbrowse.vpn.common.compose.MNEMONIC_CLIPBOARD_CLEAR_MS
import com.warrenbrowse.vpn.common.compose.SecureScreenWhileInView
import com.warrenbrowse.vpn.common.compose.createCopyToClipboardHandle
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.component.dialog.NegativeConfirmationDialog
import com.warrenbrowse.vpn.lib.ui.component.wallet.MnemonicDisplay
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.designsystem.VariantButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import org.koin.androidx.compose.koinViewModel

/**
 * Backup screen shown immediately after [WarrenWalletLoginScreen]
 * generates a fresh mnemonic via `WalletRepository.createWallet`. Mirrors the
 * desktop backup view: the phrase is shown in full alongside a copy action, and
 * a confirmation checkbox gates the Continue button.
 *
 * The step is leavable, like the desktop backup step: a user who tapped
 * "Create a new account" when they meant "I already have an account" has to be
 * able to get back to that choice. The wallet is already persisted by the time
 * this screen renders, so leaving abandons an identity nobody wrote down, which
 * is why both the in-page back and the system back go through a confirmation.
 * The login screen the user lands on already guards creating or restoring over
 * a wallet that is still present.
 *
 * The copy action flags the clip as sensitive (hidden from the clipboard
 * preview on Android 13+) and schedules the clip to be dropped a minute later,
 * matching the desktop `CopyMnemonicButton`. The mnemonic is held by
 * [WarrenWalletBackupViewModel], a NavBackStackEntry-scoped ViewModel that
 * consumes the [com.warrenbrowse.vpn.lib.repository.MnemonicCache] slot once at
 * construction and zeros its [CharArray] in `onCleared` when the back-stack
 * entry is popped. On a process restore (cache slot empty),
 * [onProcessRestoreFailure] is invoked so the host can route back to login.
 */
@Composable
fun WarrenWalletBackupScreen(
    onConfirmed: () -> Unit,
    onProcessRestoreFailure: () -> Unit,
    onDiscarded: () -> Unit,
    modifier: Modifier = Modifier,
    vm: WarrenWalletBackupViewModel = koinViewModel(),
) {
    val mnemonic = vm.mnemonic

    if (mnemonic == null) {
        // Process restore path: the cache was empty at ViewModel init. Bubble
        // up so the host EntryProvider can route back to the login screen.
        LaunchedEffect(Unit) { onProcessRestoreFailure() }
        return
    }

    // The full phrase is on screen here; block screenshots and the Recents thumbnail.
    SecureScreenWhileInView()

    var confirmLeave by remember { mutableStateOf(false) }
    // The in-page back arrow and the system back do the same thing, so a swipe
    // cannot skip the warning the arrow raises.
    BackHandler { confirmLeave = true }

    val snackbarHostState = remember { SnackbarHostState() }
    val copyToClipboard =
        createCopyToClipboardHandle(
            snackbarHostState,
            isSensitive = true,
            autoClearAfterMs = MNEMONIC_CLIPBOARD_CLEAR_MS,
        )
    var confirmed by remember { mutableStateOf(false) }
    // The backup ends with the user typing back a few words of the phrase:
    // `checkPositions` is set while that check is on screen.
    var checkPositions by remember { mutableStateOf<List<Int>?>(null) }
    var checkAnswers by remember { mutableStateOf(listOf<String>()) }
    var checkMismatch by remember { mutableStateOf(false) }

    ScaffoldWithSmallTopBar(
        modifier = modifier,
        appBarTitle =
            stringResource(
                if (checkPositions == null) R.string.wallet_backup_title
                else R.string.wallet_backup_check_title
            ),
        navigationIcon = { NavigateBackIconButton(onNavigateBack = { confirmLeave = true }) },
        snackbarHostState = snackbarHostState,
    ) { scaffoldModifier ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .then(scaffoldModifier)
                .background(MaterialTheme.colorScheme.surface)
                .verticalScroll(rememberScrollState())
                .padding(
                    start = Dimens.sideMargin,
                    end = Dimens.sideMargin,
                    top = Dimens.screenTopMargin,
                    bottom = Dimens.screenBottomMargin,
                ),
            verticalArrangement = Arrangement.spacedBy(Dimens.mediumPadding),
        ) {
            val positions = checkPositions
            if (positions != null) {
                Text(
                    text = stringResource(R.string.wallet_backup_check_body),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                positions.forEachIndexed { index, position ->
                    OutlinedTextField(
                        modifier = Modifier.fillMaxWidth(),
                        value = checkAnswers[index],
                        onValueChange = { value ->
                            checkMismatch = false
                            checkAnswers =
                                checkAnswers.mapIndexed { i, answer -> if (i == index) value else answer }
                        },
                        label = { Text(stringResource(R.string.wallet_backup_check_word, position + 1)) },
                        singleLine = true,
                        isError = checkMismatch,
                        keyboardOptions =
                            KeyboardOptions(
                                capitalization = KeyboardCapitalization.None,
                                autoCorrectEnabled = false,
                                keyboardType = KeyboardType.Password,
                            ),
                    )
                }
                if (checkMismatch) {
                    Text(
                        text = stringResource(R.string.wallet_backup_check_mismatch),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.error,
                    )
                }
                VariantButton(
                    modifier = Modifier.fillMaxWidth(),
                    onClick = {
                        if (backupCheckPasses(mnemonic.phrase, positions, checkAnswers)) {
                            onConfirmed()
                        } else {
                            checkMismatch = true
                        }
                    },
                    text = stringResource(R.string.cont),
                    isEnabled = checkAnswers.all { it.isNotBlank() },
                )
                PrimaryButton(
                    modifier = Modifier.fillMaxWidth(),
                    onClick = { checkPositions = null },
                    text = stringResource(R.string.back),
                )
            } else {
                Text(
                    text = stringResource(R.string.wallet_backup_body),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                )

                Text(
                    text = stringResource(R.string.wallet_backup_warning),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.error,
                )

                MnemonicDisplay(phrase = mnemonic.phrase, alwaysRevealed = true)

                PrimaryButton(
                    modifier = Modifier.fillMaxWidth(),
                    onClick = { copyToClipboard(mnemonic.phrase, null) },
                    text = stringResource(R.string.copy),
                )

                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable { confirmed = !confirmed },
                    horizontalArrangement = Arrangement.spacedBy(Dimens.buttonSpacing),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Checkbox(checked = confirmed, onCheckedChange = { confirmed = it })
                    Text(
                        text = stringResource(R.string.wallet_backup_confirm_cta),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurface,
                    )
                }

                VariantButton(
                    modifier = Modifier.fillMaxWidth(),
                    onClick = {
                        val positions = pickBackupCheckPositions(mnemonic.phrase.trim().split(' ').size)
                        checkPositions = positions
                        checkAnswers = positions.map { "" }
                        checkMismatch = false
                    },
                    text = stringResource(R.string.cont),
                    isEnabled = confirmed,
                )
            }
        }
    }

    if (confirmLeave) {
        NegativeConfirmationDialog(
            message = stringResource(R.string.wallet_backup_leave_confirm),
            confirmationText = stringResource(R.string.wallet_backup_leave_action),
            onConfirm = {
                confirmLeave = false
                onDiscarded()
            },
            onBack = { confirmLeave = false },
        )
    }
}
