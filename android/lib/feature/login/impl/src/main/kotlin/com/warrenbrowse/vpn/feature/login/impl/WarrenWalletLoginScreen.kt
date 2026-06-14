package com.warrenbrowse.vpn.feature.login.impl

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.ui.component.wallet.BiometricPromptAuthorizer
import com.warrenbrowse.vpn.lib.ui.component.wallet.MnemonicInput
import com.warrenbrowse.vpn.lib.ui.resource.R
import org.koin.androidx.compose.koinViewModel

/**
 * D.5 wallet entry-point screen.
 *
 * On first launch with no persisted wallet, the user is offered two
 * branches:
 *   - `Generate recovery phrase` -> emit `BackupGeneratedMnemonic` so
 *     the host NavController routes to `WarrenWalletBackupScreen`.
 *   - `Restore from recovery phrase` -> inline `MnemonicInput` for the
 *     12-word phrase, then `WarrenWalletViewModel.importWallet`, which
 *     emits `WalletReady` on success.
 *
 * The ViewModel owns the repository interaction and one-shot event
 * dispatch (Channel) so re-emission on config change does not
 * re-navigate. Navigation routing itself is owned by the app NavGraph;
 * this screen only forwards events.
 */
@Composable
fun WarrenWalletLoginScreen(
    onWalletCreated: (Mnemonic) -> Unit,
    onWalletReady: () -> Unit,
    modifier: Modifier = Modifier,
    vm: WarrenWalletViewModel = koinViewModel(),
) {
    val state by vm.state.collectAsStateWithLifecycle()

    // Used only when hardware-bound keystore auth is enabled: the encrypt step
    // at wallet create/import is gated by a CryptoObject prompt. With the flag
    // off the authorizer is ignored, so no prompt appears at creation.
    val activity = LocalContext.current as FragmentActivity
    val authorizer = remember(activity) { BiometricPromptAuthorizer(activity) }

    var importMode by remember { mutableStateOf(false) }
    var importPhrase by remember { mutableStateOf("") }
    var inlineError by remember { mutableStateOf<String?>(null) }

    LaunchedEffect(Unit) {
        vm.events.collect { event ->
            when (event) {
                is WarrenWalletEvent.BackupGeneratedMnemonic -> onWalletCreated(event.mnemonic)
                WarrenWalletEvent.WalletReady -> onWalletReady()
                is WarrenWalletEvent.Error -> inlineError = event.message
            }
        }
    }

    Column(
        modifier = modifier
            .fillMaxSize()
            .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            text = stringResource(R.string.wallet_login_title),
            style = MaterialTheme.typography.headlineMedium,
        )
        Text(
            text = if (importMode) {
                stringResource(R.string.wallet_login_import_prompt)
            } else {
                stringResource(R.string.wallet_login_subtitle)
            },
            style = MaterialTheme.typography.bodyMedium,
        )

        if (importMode) {
            MnemonicInput(
                onPhraseChange = { phrase ->
                    importPhrase = phrase
                    inlineError = null
                },
            )
            inlineError?.let { msg ->
                Text(text = msg, color = MaterialTheme.colorScheme.error)
            }
            Button(
                onClick = { vm.importWallet(importPhrase, authorizer) },
                enabled = importPhrase.isNotBlank(),
            ) {
                Text(text = stringResource(R.string.wallet_import_cta))
            }
            OutlinedButton(onClick = { importMode = false; inlineError = null }) {
                Text(text = stringResource(R.string.back))
            }
        } else {
            inlineError?.let { msg ->
                Text(text = msg, color = MaterialTheme.colorScheme.error)
            }
            Button(onClick = { vm.createWallet(authorizer) }) {
                Text(text = stringResource(R.string.wallet_create_cta))
            }
            OutlinedButton(onClick = { importMode = true }) {
                Text(text = stringResource(R.string.wallet_import_title))
            }
        }

        // `state` is observed so the host can still react if the wallet
        // gets persisted out-of-band (e.g. import succeeded just before
        // a config change re-composed us). The ViewModel emits a
        // `WalletReady` event in that path too, so navigation is event-
        // driven; the observation here just keeps the recomposition
        // model honest.
        @Suppress("UNUSED_EXPRESSION") state
    }
}
