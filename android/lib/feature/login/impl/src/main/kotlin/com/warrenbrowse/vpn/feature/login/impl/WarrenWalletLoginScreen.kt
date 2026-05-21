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
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.ui.component.wallet.MnemonicInput
import kotlinx.coroutines.launch

/**
 * D.5 wallet entry-point screen.
 *
 * On first launch with no persisted wallet, the user is offered two
 * branches:
 *   - `Generate recovery phrase` -> route to a Backup screen that displays
 *     the freshly-generated mnemonic via `MnemonicDisplay`.
 *   - `Restore from recovery phrase` -> inline `MnemonicInput` for the
 *     12-word phrase, then `WalletRepository.importWallet`.
 *
 * Once the repository transitions to [WalletState.Ready] the navigation
 * graph (NavHost) is responsible for routing forward to the home screen.
 * That orchestration lives in the app module; this screen only owns the
 * branch decision and the import inline flow.
 */
@Composable
fun WarrenWalletLoginScreen(
    walletRepository: WalletRepository,
    onWalletCreated: (Mnemonic) -> Unit,
    onWalletReady: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val state by walletRepository.state.collectAsState()
    val scope = rememberCoroutineScope()

    var importMode by remember { mutableStateOf(false) }
    var importPhrase by remember { mutableStateOf("") }
    var importError by remember { mutableStateOf<String?>(null) }

    LaunchedEffect(state) {
        if (state is WalletState.Ready) onWalletReady()
    }

    Column(
        modifier = modifier
            .fillMaxSize()
            .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            text = "Warren VPN",
            style = MaterialTheme.typography.headlineMedium,
        )
        Text(
            text = if (importMode) {
                "Enter your 12-word recovery phrase"
            } else {
                "Create a new wallet or restore from an existing recovery phrase"
            },
            style = MaterialTheme.typography.bodyMedium,
        )

        if (importMode) {
            MnemonicInput(
                onPhraseChange = { phrase ->
                    importPhrase = phrase
                    importError = null
                },
            )
            importError?.let { msg ->
                Text(text = msg, color = MaterialTheme.colorScheme.error)
            }
            Button(
                onClick = {
                    scope.launch {
                        try {
                            walletRepository.importWallet(Mnemonic(importPhrase))
                        } catch (e: IllegalArgumentException) {
                            importError = "Invalid recovery phrase"
                        } catch (e: Exception) {
                            importError = "Import failed: ${e.message}"
                        }
                    }
                },
                enabled = importPhrase.isNotBlank(),
            ) {
                Text(text = "Restore wallet")
            }
            OutlinedButton(onClick = { importMode = false }) {
                Text(text = "Back")
            }
        } else {
            Button(
                onClick = {
                    scope.launch {
                        val mnemonic = walletRepository.createWallet()
                        onWalletCreated(mnemonic)
                    }
                },
            ) {
                Text(text = "Generate recovery phrase")
            }
            OutlinedButton(onClick = { importMode = true }) {
                Text(text = "Restore from recovery phrase")
            }
        }
    }
}
