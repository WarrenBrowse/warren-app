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
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.warrenbrowse.vpn.common.compose.SecureScreenWhileInView
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.WarrenHelpLink
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.component.dialog.NegativeConfirmationDialog
import com.warrenbrowse.vpn.lib.ui.component.wallet.BiometricPromptAuthorizer
import com.warrenbrowse.vpn.lib.ui.component.wallet.MnemonicInput
import com.warrenbrowse.vpn.lib.ui.component.wallet.countMnemonicWords
import com.warrenbrowse.vpn.lib.ui.designsystem.VariantButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import org.koin.androidx.compose.koinViewModel

/**
 * Restore another wallet from its recovery phrase without erasing the current
 * one first (desktop `RestoreMnemonicView`, pushed from the keys view).
 *
 * The import overwrites the persisted wallet, so it goes through an explicit
 * confirmation: unlike the login screen, the user reaching this screen already
 * holds an identity that only its own phrase can bring back.
 */
@Composable
fun WarrenRestoreMnemonicScreen(
    onRestored: () -> Unit,
    onNavigateBack: () -> Unit,
    modifier: Modifier = Modifier,
    vm: WarrenWalletViewModel = koinViewModel(),
) {
    val activity = LocalContext.current as FragmentActivity
    val authorizer = remember(activity) { BiometricPromptAuthorizer(activity) }

    val busy by vm.busy.collectAsStateWithLifecycle()
    val phrase by vm.importPhrase.collectAsStateWithLifecycle()
    var inlineError by remember { mutableStateOf<String?>(null) }
    var confirmOverwrite by remember { mutableStateOf(false) }

    val wrongWordCountError = stringResource(R.string.wallet_import_error_word_count)
    val invalidPhraseError = stringResource(R.string.wallet_import_error_invalid)
    val createFailedError = stringResource(R.string.wallet_create_error)

    LaunchedEffect(Unit) {
        vm.events.collect { event ->
            when (event) {
                WarrenWalletEvent.WalletReady -> onRestored()
                is WarrenWalletEvent.Error ->
                    inlineError =
                        when (event.reason) {
                            WalletErrorReason.WrongWordCount -> wrongWordCountError
                            WalletErrorReason.InvalidPhrase -> invalidPhraseError
                            WalletErrorReason.CreateFailed -> createFailedError
                        }
                // This screen never mints a wallet.
                is WarrenWalletEvent.BackupGeneratedMnemonic -> Unit
            }
        }
    }

    // The phrase being typed is on screen; block screenshots and Recents.
    SecureScreenWhileInView()

    ScaffoldWithSmallTopBar(
        modifier = modifier,
        appBarTitle = stringResource(R.string.wallet_import_title),
        navigationIcon = { NavigateBackIconButton(onNavigateBack = onNavigateBack) },
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
                text = stringResource(R.string.wallet_login_import_prompt),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            MnemonicInput(
                phrase = phrase,
                onPhraseChange = {
                    vm.setImportPhrase(it)
                    inlineError = null
                },
            )

            inlineError?.let { message ->
                Text(
                    text = message,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                )
                WarrenHelpLink()
            }

            val wordCount = countMnemonicWords(phrase)
            VariantButton(
                modifier = Modifier.fillMaxWidth(),
                onClick = { confirmOverwrite = true },
                text = stringResource(R.string.wallet_import_cta),
                isEnabled = wordCount == 12 || wordCount == 24,
                isLoading = busy,
            )
        }
    }

    if (confirmOverwrite) {
        NegativeConfirmationDialog(
            message = stringResource(R.string.wallet_restore_overwrite_confirm),
            confirmationText = stringResource(R.string.wallet_import_cta),
            onConfirm = {
                confirmOverwrite = false
                vm.importWallet(authorizer)
            },
            onBack = { confirmOverwrite = false },
        )
    }
}
