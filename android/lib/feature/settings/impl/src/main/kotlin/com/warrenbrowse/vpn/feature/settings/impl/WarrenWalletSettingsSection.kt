package com.warrenbrowse.vpn.feature.settings.impl

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.fragment.app.FragmentActivity
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.WalletAuthorizationDeniedException
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.ui.component.wallet.BiometricPromptAuthorizer
import com.warrenbrowse.vpn.lib.ui.component.wallet.MnemonicDisplay
import kotlinx.coroutines.launch

/**
 * Settings section that exposes the two sensitive wallet operations:
 *   - "View recovery phrase" -> BiometricPrompt -> WalletRepository.unlock
 *     -> render the cleartext mnemonic via `MnemonicDisplay`.
 *   - "Erase wallet" -> confirmation AlertDialog -> WalletRepository.erase.
 *
 * The reveal is ephemeral: tapping outside the dialog hides the mnemonic
 * and the Compose state is dropped on the next recomposition, so the
 * plaintext does not survive into a logger or a captured screenshot
 * unless the user actively kept it on screen.
 *
 * Must be hosted inside a [FragmentActivity] for `BiometricPrompt` to
 * work. The typical caller is `MainActivity` (extends FragmentActivity
 * via D.5 step 3).
 */
@Composable
fun WarrenWalletSettingsSection(
    activity: FragmentActivity,
    walletRepository: WalletRepository,
    modifier: Modifier = Modifier,
) {
    val state by walletRepository.state.collectAsState()
    val scope = rememberCoroutineScope()
    var viewMnemonic by remember { mutableStateOf<Mnemonic?>(null) }
    var confirmErase by remember { mutableStateOf(false) }
    var viewError by remember { mutableStateOf<String?>(null) }

    LaunchedEffect(state) {
        if (state is WalletState.Absent) {
            // Wallet erased elsewhere - clear any cached reveal.
            viewMnemonic = null
        }
    }

    Column(modifier = modifier.fillMaxWidth().padding(16.dp)) {
        Text(
            text = "Wallet",
            style = MaterialTheme.typography.titleMedium,
        )

        // Wallet identity: pubkey + state hint. Tap the pubkey row to
        // reveal the full 64-char hex (default view truncates for
        // readability). Pubkey is not sensitive (it's the public key);
        // we still avoid auto-displaying the full string because the
        // truncated form keeps the Settings UI compact.
        var pubkeyExpanded by remember { mutableStateOf(false) }
        when (val s = state) {
            is WalletState.Ready -> {
                val full = s.pubkey.value
                val display = if (pubkeyExpanded) full else full.take(8) + "…" + full.takeLast(8)
                Text(
                    text = "Pubkey: $display",
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(top = 4.dp, bottom = 8.dp)
                        .clickable { pubkeyExpanded = !pubkeyExpanded },
                )
            }
            WalletState.Locked -> {
                Text(
                    text = "Wallet locked. Authenticate to view.",
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(top = 4.dp, bottom = 8.dp),
                )
            }
            WalletState.Absent -> {
                Text(
                    text = "No wallet on this device.",
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(top = 4.dp, bottom = 8.dp),
                )
            }
        }

        viewError?.let { msg ->
            Text(text = msg, color = MaterialTheme.colorScheme.error)
        }

        OutlinedButton(
            modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
            onClick = {
                scope.launch {
                    try {
                        val authorizer = BiometricPromptAuthorizer(activity)
                        viewMnemonic = walletRepository.unlock(
                            authorizer = authorizer,
                            reason = "Confirm to view your recovery phrase",
                        )
                        viewError = null
                    } catch (e: WalletAuthorizationDeniedException) {
                        viewError = "Authentication required to view your recovery phrase"
                    } catch (e: Exception) {
                        Logger.w(throwable = e) { "wallet unlock failed" }
                        viewError = "Unable to read wallet: ${e.message}"
                    }
                }
            },
        ) { Text("View recovery phrase") }

        OutlinedButton(
            modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
            onClick = { confirmErase = true },
        ) { Text("Erase wallet") }
    }

    viewMnemonic?.let { mnemonic ->
        AlertDialog(
            onDismissRequest = { viewMnemonic = null },
            title = { Text("Your recovery phrase") },
            text = { MnemonicDisplay(phrase = mnemonic.phrase) },
            confirmButton = {
                TextButton(onClick = { viewMnemonic = null }) { Text("Done") }
            },
        )
    }

    if (confirmErase) {
        AlertDialog(
            onDismissRequest = { confirmErase = false },
            title = { Text("Erase this wallet?") },
            text = {
                Text(
                    "This cannot be undone. You will need your recovery phrase to access " +
                        "this account again."
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    confirmErase = false
                    scope.launch { walletRepository.erase() }
                }) { Text("Erase") }
            },
            dismissButton = {
                TextButton(onClick = { confirmErase = false }) { Text("Cancel") }
            },
        )
    }
}
