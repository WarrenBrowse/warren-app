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
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.unit.dp
import androidx.fragment.app.FragmentActivity
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.model.wallet.shortWarrenAddress
import com.warrenbrowse.vpn.lib.repository.WalletAuthorizationDeniedException
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.ui.component.wallet.BiometricPromptAuthorizer
import com.warrenbrowse.vpn.lib.ui.component.wallet.MnemonicDisplay
import com.warrenbrowse.vpn.lib.ui.resource.R
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
 * work. The typical caller is `MainActivity` (extends FragmentActivity).
 */
@Composable
fun WarrenWalletSettingsSection(
    activity: FragmentActivity,
    walletRepository: WalletRepository,
    modifier: Modifier = Modifier,
) {
    val state by walletRepository.state.collectAsState()
    val scope = rememberCoroutineScope()
    // LocalClipboardManager is deprecated for LocalClipboard (suspend).
    // The replacement requires plumbing a CoroutineScope around every
    // clipboard.setText call, which would balloon the diff. The
    // legacy API still works on every Android API the project targets;
    // a focused migration is tracked as a future task.
    @Suppress("DEPRECATION")
    val clipboard = LocalClipboardManager.current
    var viewMnemonic by remember { mutableStateOf<Mnemonic?>(null) }
    var confirmErase by remember { mutableStateOf(false) }
    var viewError by remember { mutableStateOf<String?>(null) }
    var copyHint by remember { mutableStateOf<String?>(null) }

    val viewPhraseReason = stringResource(R.string.wallet_biometric_view_phrase_reason)
    val pubkeyCopiedHint = stringResource(R.string.wallet_settings_pubkey_copied)
    val authRequiredError = stringResource(R.string.wallet_settings_auth_required_view_phrase)
    val unableToReadPrefix = stringResource(R.string.wallet_settings_unable_to_read)

    LaunchedEffect(state) {
        if (state is WalletState.Absent) {
            // Wallet erased elsewhere - clear any cached reveal.
            viewMnemonic = null
        }
    }

    Column(modifier = modifier.fillMaxWidth().padding(16.dp)) {
        Text(
            text = stringResource(R.string.wallet_settings_section),
            style = MaterialTheme.typography.titleMedium,
        )

        // Wallet identity: address + state hint. Tap the row to copy the
        // full Warren SS58 address to the clipboard (the address is the
        // user's shareable public identity - not a secret). The default
        // display shows the Polkadot short form (first 6 + … + last 6)
        // for compactness.
        when (val s = state) {
            is WalletState.Ready -> {
                val full = s.pubkey.value
                val truncated = full.shortWarrenAddress()
                Text(
                    text = stringResource(R.string.wallet_settings_pubkey_tap_to_copy, truncated),
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(top = 4.dp, bottom = 8.dp)
                        .clickable {
                            clipboard.setText(AnnotatedString(full))
                            copyHint = pubkeyCopiedHint
                        },
                )
            }
            WalletState.Locked -> {
                Text(
                    text = stringResource(R.string.wallet_settings_locked_hint),
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(top = 4.dp, bottom = 8.dp),
                )
            }
            WalletState.Absent -> {
                Text(
                    text = stringResource(R.string.wallet_settings_absent_hint),
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(top = 4.dp, bottom = 8.dp),
                )
            }
        }

        copyHint?.let { hint ->
            Text(
                text = hint,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(bottom = 8.dp),
            )
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
                            reason = viewPhraseReason,
                        )
                        viewError = null
                    } catch (e: WalletAuthorizationDeniedException) {
                        viewError = authRequiredError
                    } catch (e: Exception) {
                        Logger.w(throwable = e) { "wallet unlock failed" }
                        viewError = "$unableToReadPrefix ${e.message}"
                    }
                }
            },
        ) { Text(stringResource(R.string.wallet_settings_view_phrase)) }

        OutlinedButton(
            modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
            onClick = { confirmErase = true },
        ) { Text(stringResource(R.string.wallet_settings_erase)) }
    }

    viewMnemonic?.let { mnemonic ->
        AlertDialog(
            onDismissRequest = { viewMnemonic = null },
            title = { Text(stringResource(R.string.wallet_settings_recovery_phrase_title)) },
            text = { MnemonicDisplay(phrase = mnemonic.phrase) },
            confirmButton = {
                TextButton(onClick = { viewMnemonic = null }) {
                    Text(stringResource(R.string.wallet_settings_done))
                }
            },
        )
    }

    if (confirmErase) {
        AlertDialog(
            onDismissRequest = { confirmErase = false },
            title = { Text(stringResource(R.string.wallet_settings_erase_confirm_title)) },
            text = {
                Text(stringResource(R.string.wallet_settings_erase_confirm_description))
            },
            confirmButton = {
                TextButton(onClick = {
                    confirmErase = false
                    scope.launch { walletRepository.erase() }
                }) { Text(stringResource(R.string.wallet_settings_erase_confirm_action)) }
            },
            dismissButton = {
                TextButton(onClick = { confirmErase = false }) {
                    Text(stringResource(R.string.cancel))
                }
            },
        )
    }
}
