package com.warrenbrowse.vpn.feature.login.impl

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.ui.component.wallet.MnemonicDisplay

/**
 * D.5 backup screen shown immediately after [WarrenWalletLoginScreen]
 * generates a fresh mnemonic via `WalletRepository.createWallet`.
 *
 * Renders the cleartext phrase via `MnemonicDisplay` (blur+reveal, no
 * copy CTA) plus a single confirmation button. The user is expected to
 * write the phrase down on paper before tapping `I have written it down`.
 *
 * After the confirmation the navigation graph routes forward to the home
 * screen; this composable does not orchestrate that transition - it owns
 * the display + the confirm callback only.
 */
@Composable
fun WarrenWalletBackupScreen(
    mnemonic: Mnemonic,
    onConfirmed: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Surface(
        color = MaterialTheme.colorScheme.background,
        modifier = modifier.fillMaxSize(),
    ) {
        Column(
            modifier = Modifier.padding(24.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text(
                text = "Write down your recovery phrase",
                style = MaterialTheme.typography.headlineSmall,
            )
            Text(
                text = "Anyone with these 12 words can access your account. " +
                    "Never share them or store them online.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.error,
            )

            MnemonicDisplay(phrase = mnemonic.phrase)

            Text(
                text = "Warren does not provide a 'copy to clipboard' button on " +
                    "purpose - clipboard contents leak to other apps. Use pen " +
                    "and paper, or an offline password manager.",
                style = MaterialTheme.typography.bodySmall,
                textAlign = TextAlign.Start,
            )

            Button(onClick = onConfirmed) {
                Text(text = "I have written it down")
            }
        }
    }
}
