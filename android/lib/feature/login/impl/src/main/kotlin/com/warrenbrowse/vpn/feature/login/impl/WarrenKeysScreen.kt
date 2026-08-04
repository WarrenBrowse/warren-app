package com.warrenbrowse.vpn.feature.login.impl

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material3.Checkbox
import androidx.compose.material3.Icon
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
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.common.compose.MNEMONIC_CLIPBOARD_CLEAR_MS
import com.warrenbrowse.vpn.common.compose.SecureScreenWhileInView
import com.warrenbrowse.vpn.common.compose.createCopyToClipboardHandle
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.component.wallet.MnemonicDisplay
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.designsystem.VariantButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha20
import org.koin.androidx.compose.koinViewModel

/**
 * Recovery-phrase screen reached from the account page after the biometric
 * unlock, mirroring the desktop `KeysView`: the handling warning, the phrase,
 * a copy action, an acknowledgement gate on the way out, and a non-destructive
 * route to restoring another wallet.
 *
 * It is a destination rather than a dialog so the back gesture, the top-bar
 * chevron and the Done button all leave the same way, and so the restore step
 * can be pushed on top of it.
 *
 * The phrase arrives through the [com.warrenbrowse.vpn.lib.repository.MnemonicCache]
 * slot staged by the unlock, never through the NavKey: Compose Navigation
 * persists NavKeys into the saved-state Bundle.
 */
@Composable
fun WarrenKeysScreen(
    onDone: () -> Unit,
    onRestoreOtherPhrase: () -> Unit,
    onProcessRestoreFailure: () -> Unit,
    modifier: Modifier = Modifier,
    vm: WarrenWalletBackupViewModel = koinViewModel(),
) {
    val mnemonic = vm.mnemonic

    if (mnemonic == null) {
        // The staged phrase is gone (process kill, or the slot was drained out
        // of band). The account screen unlocks again on the next tap.
        LaunchedEffect(Unit) { onProcessRestoreFailure() }
        return
    }

    // The full phrase is on screen here; block screenshots and the Recents thumbnail.
    SecureScreenWhileInView()

    val snackbarHostState = remember { SnackbarHostState() }
    val copyToClipboard =
        createCopyToClipboardHandle(
            snackbarHostState,
            isSensitive = true,
            autoClearAfterMs = MNEMONIC_CLIPBOARD_CLEAR_MS,
        )
    var acknowledged by remember { mutableStateOf(false) }

    ScaffoldWithSmallTopBar(
        modifier = modifier,
        appBarTitle = stringResource(R.string.wallet_settings_recovery_phrase_title),
        navigationIcon = { NavigateBackIconButton(onNavigateBack = onDone) },
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

            RestoreOtherPhraseRow(onClick = onRestoreOtherPhrase)

            Row(
                modifier = Modifier.fillMaxWidth().clickable { acknowledged = !acknowledged },
                horizontalArrangement = Arrangement.spacedBy(Dimens.buttonSpacing),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Checkbox(checked = acknowledged, onCheckedChange = { acknowledged = it })
                Text(
                    text = stringResource(R.string.wallet_backup_confirm_cta),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                )
            }

            VariantButton(
                modifier = Modifier.fillMaxWidth(),
                onClick = onDone,
                text = stringResource(R.string.wallet_settings_done),
                isEnabled = acknowledged,
            )
        }
    }
}

/** Navigation, not a command: outlined row with a chevron, like BackupPhraseRow. */
@Composable
private fun RestoreOtherPhraseRow(onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .border(
                border = BorderStroke(
                    width = Dimens.outLineButtonBorderWidth,
                    color = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha20),
                ),
                shape = RoundedCornerShape(12.dp),
            )
            .clickable(onClick = onClick)
            .padding(horizontal = Dimens.mediumPadding, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = stringResource(R.string.wallet_keys_restore_other),
            style = MaterialTheme.typography.titleSmall,
            color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.weight(1f),
        )
        Icon(
            imageVector = Icons.AutoMirrored.Rounded.KeyboardArrowRight,
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
