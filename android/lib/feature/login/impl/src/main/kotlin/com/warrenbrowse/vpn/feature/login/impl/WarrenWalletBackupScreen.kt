package com.warrenbrowse.vpn.feature.login.impl

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
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import com.warrenbrowse.vpn.lib.ui.component.wallet.MnemonicDisplay
import com.warrenbrowse.vpn.lib.ui.designsystem.VariantButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import org.koin.androidx.compose.koinViewModel

/**
 * D.5 backup screen shown immediately after [WarrenWalletLoginScreen]
 * generates a fresh mnemonic via `WalletRepository.createWallet`.
 *
 * Renders the cleartext phrase via `MnemonicDisplay` (blur+reveal, no copy
 * CTA) plus a single confirmation button. The user is expected to write the
 * phrase down on paper before tapping `I have written it down`.
 *
 * The mnemonic is held by [WarrenWalletBackupViewModel], a
 * NavBackStackEntry-scoped ViewModel that consumes the
 * [com.warrenbrowse.vpn.lib.repository.MnemonicCache] slot once at
 * construction and zeros its [CharArray] in `onCleared` when the back-stack
 * entry is popped. On a process restore (cache slot empty),
 * [onProcessRestoreFailure] is invoked so the host can route back to login.
 */
@Composable
fun WarrenWalletBackupScreen(
    onConfirmed: () -> Unit,
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

    Column(
        modifier = modifier
            .fillMaxSize()
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
            text = stringResource(R.string.wallet_backup_title),
            style = MaterialTheme.typography.headlineSmall,
            color = MaterialTheme.colorScheme.onSurface,
        )
        Text(
            text = stringResource(R.string.wallet_backup_warning),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.error,
        )

        MnemonicDisplay(phrase = mnemonic.phrase)

        Text(
            text = stringResource(R.string.wallet_backup_clipboard_note),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        VariantButton(
            modifier = Modifier.fillMaxWidth().padding(top = Dimens.mediumPadding),
            onClick = onConfirmed,
            text = stringResource(R.string.wallet_backup_confirm_cta),
        )
    }
}
