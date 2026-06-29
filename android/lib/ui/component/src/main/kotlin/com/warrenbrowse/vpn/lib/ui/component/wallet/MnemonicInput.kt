package com.warrenbrowse.vpn.lib.ui.component.wallet

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.lib.ui.resource.R

/**
 * 12 (or 24) word BIP39 mnemonic input.
 *
 * Single paste-friendly multi-line field that mirrors the desktop
 * `MnemonicTextarea`: the user pastes or types the whole phrase separated by
 * spaces and a live word counter gives BIP39-validity feedback. This replaces
 * the previous per-word grid, where focus did not reliably advance between
 * cells and a pasted phrase landed entirely in the first box.
 *
 * The daemon performs the real wordlist + checksum validation
 * (`WarrenJni.importMnemonic`); the counter is UI feedback only. [onPhraseChange]
 * receives the canonicalized phrase (trimmed, lowercased, single-spaced) so the
 * caller can hand it straight to the daemon.
 */
@Composable
fun MnemonicInput(
    onPhraseChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    wordCount: Int = 12,
    initialPhrase: String = "",
) {
    require(wordCount == 12 || wordCount == 24) {
        "BIP39 mnemonic must be 12 or 24 words, got $wordCount"
    }

    var raw by remember { mutableStateOf(initialPhrase) }
    val count = remember(raw) { countMnemonicWords(raw) }

    Column(
        modifier = modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        OutlinedTextField(
            value = raw,
            onValueChange = { newValue ->
                raw = newValue
                onPhraseChange(normalizeMnemonic(newValue))
            },
            modifier = Modifier.fillMaxWidth(),
            placeholder = { Text(stringResource(R.string.wallet_import_placeholder)) },
            textStyle = MaterialTheme.typography.bodyLarge.copy(fontFamily = FontFamily.Monospace),
            minLines = 3,
            maxLines = 5,
            keyboardOptions = KeyboardOptions(
                capitalization = KeyboardCapitalization.None,
                autoCorrectEnabled = false,
                keyboardType = KeyboardType.Text,
                imeAction = ImeAction.Done,
            ),
        )
        Text(
            text = stringResource(R.string.wallet_import_word_count, count, wordCount),
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

/** Number of whitespace-separated words in [input]. */
fun countMnemonicWords(input: String): Int =
    input.trim().split(WHITESPACE).count { it.isNotEmpty() }

/**
 * Canonicalizes a pasted BIP39 phrase before handing it to the daemon: trims,
 * lowercases, and collapses runs of whitespace to single spaces. Mirrors the
 * desktop `normalizeMnemonic` so both clients accept identical input.
 */
fun normalizeMnemonic(input: String): String =
    input.trim().lowercase().split(WHITESPACE).filter { it.isNotEmpty() }.joinToString(" ")

private val WHITESPACE = Regex("\\s+")
