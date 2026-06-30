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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithTopBar
import com.warrenbrowse.vpn.lib.ui.component.wallet.BiometricPromptAuthorizer
import com.warrenbrowse.vpn.lib.ui.component.wallet.MnemonicInput
import com.warrenbrowse.vpn.lib.ui.component.wallet.countMnemonicWords
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryTextButton
import com.warrenbrowse.vpn.lib.ui.designsystem.VariantButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import org.koin.androidx.compose.koinViewModel

/**
 * Wallet entry-point screen. Mirrors the desktop login UX: the Warren mark
 * sits in the top bar (like the desktop AppMainHeader), then a short intro and
 * two full-width choices.
 *
 *   - `Generate recovery phrase` (positive/green CTA) emits
 *     `BackupGeneratedMnemonic` so the host NavController routes to
 *     `WarrenWalletBackupScreen`.
 *   - `Restore from recovery phrase` (primary/blue) opens an inline
 *     `MnemonicInput` for the 12-word phrase, then
 *     `WarrenWalletViewModel.importWallet`, which emits `WalletReady` on
 *     success.
 *
 * The ViewModel owns the repository interaction and one-shot event dispatch
 * (Channel) so re-emission on config change does not re-navigate.
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

    ScaffoldWithTopBar(
        modifier = modifier,
        topBarColor = MaterialTheme.colorScheme.surface,
        onSettingsClicked = null,
        onAccountClicked = null,
    ) { pv ->
        Column(
            modifier = Modifier
                .fillMaxSize()
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
                    onPhraseChange = { phrase ->
                        importPhrase = phrase
                        inlineError = null
                    },
                )
                inlineError?.let { msg ->
                    Text(
                        text = msg,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
                        textAlign = TextAlign.Center,
                    )
                }

                Column(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(Dimens.buttonSpacing),
                ) {
                    val importWordCount = countMnemonicWords(importPhrase)
                    VariantButton(
                        modifier = Modifier.fillMaxWidth(),
                        onClick = { vm.importWallet(importPhrase, authorizer) },
                        text = stringResource(R.string.wallet_import_cta),
                        isEnabled = importWordCount == 12 || importWordCount == 24,
                    )
                    PrimaryTextButton(
                        onClick = { importMode = false; inlineError = null },
                        text = stringResource(R.string.back),
                    )
                }
            } else {
                Text(
                    text = stringResource(R.string.onboarding_welcome_title),
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
                inlineError?.let { msg ->
                    Text(
                        text = msg,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
                        textAlign = TextAlign.Center,
                    )
                }

                Column(
                    modifier = Modifier.fillMaxWidth().padding(top = Dimens.mediumPadding),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(Dimens.buttonSpacing),
                ) {
                    VariantButton(
                        modifier = Modifier.fillMaxWidth(),
                        onClick = { vm.createWallet(authorizer) },
                        text = stringResource(R.string.wallet_create_cta),
                    )
                    PrimaryButton(
                        modifier = Modifier.fillMaxWidth(),
                        onClick = { importMode = true },
                        text = stringResource(R.string.wallet_import_title),
                    )
                }
            }

            // `state` is observed so the host can react if the wallet gets
            // persisted out-of-band (e.g. import succeeded just before a config
            // change recomposed us). The ViewModel emits a `WalletReady` event
            // in that path too, so navigation stays event-driven.
            @Suppress("UNUSED_EXPRESSION") state
        }
    }
}
