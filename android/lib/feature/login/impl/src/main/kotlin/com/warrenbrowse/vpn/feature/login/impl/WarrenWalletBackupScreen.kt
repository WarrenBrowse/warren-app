package com.warrenbrowse.vpn.feature.login.impl

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
import com.warrenbrowse.vpn.common.compose.createCopyToClipboardHandle
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.component.wallet.MnemonicDisplay
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.designsystem.VariantButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import org.koin.androidx.compose.koinViewModel

/**
 * D.5 backup screen shown immediately after [WarrenWalletLoginScreen]
 * generates a fresh mnemonic via `WalletRepository.createWallet`. Mirrors the
 * desktop backup view: the phrase is shown in full alongside a copy action, and
 * a confirmation checkbox gates the Continue button. The top bar exposes a back
 * affordance (returns to the login choice).
 *
 * The copy action flags the clip as sensitive (hidden from the clipboard
 * preview on Android 13+). The mnemonic is held by
 * [WarrenWalletBackupViewModel], a NavBackStackEntry-scoped ViewModel that
 * consumes the [com.warrenbrowse.vpn.lib.repository.MnemonicCache] slot once at
 * construction and zeros its [CharArray] in `onCleared` when the back-stack
 * entry is popped. On a process restore (cache slot empty),
 * [onProcessRestoreFailure] is invoked so the host can route back to login.
 */
@Composable
fun WarrenWalletBackupScreen(
    onConfirmed: () -> Unit,
    onNavigateBack: () -> Unit,
    onProcessRestoreFailure: () -> Unit,
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

    val snackbarHostState = remember { SnackbarHostState() }
    val copyToClipboard = createCopyToClipboardHandle(snackbarHostState, isSensitive = true)
    var confirmed by remember { mutableStateOf(false) }

    ScaffoldWithSmallTopBar(
        modifier = modifier,
        appBarTitle = stringResource(R.string.wallet_backup_title),
        navigationIcon = { NavigateBackIconButton(onNavigateBack = onNavigateBack) },
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
                onClick = onConfirmed,
                text = stringResource(R.string.cont),
                isEnabled = confirmed,
            )
        }
    }
}
