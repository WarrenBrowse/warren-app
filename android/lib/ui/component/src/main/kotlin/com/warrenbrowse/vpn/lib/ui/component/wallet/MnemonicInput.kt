package com.warrenbrowse.vpn.lib.ui.component.wallet

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp

/**
 * 12-word BIP39 mnemonic input. Renders a 6x2 grid (or 8x3 for 24-word
 * variant) of [OutlinedTextField]s, each pre-filled with the index. The
 * caller observes the concatenated phrase via [onPhraseChange] and can
 * validate against `bip39::Mnemonic::parse` via `WarrenJni.importMnemonic`.
 *
 * Compose preview deliberately omitted - the parent screen owns the
 * composition root (see D.5 LoginScreen / SignupWizard rewrite).
 */
@Composable
fun MnemonicInput(
    wordCount: Int = 12,
    initialWords: List<String> = List(wordCount) { "" },
    onPhraseChange: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    require(wordCount == 12 || wordCount == 24) {
        "BIP39 mnemonic must be 12 or 24 words, got $wordCount"
    }
    require(initialWords.size == wordCount) {
        "initialWords size ${initialWords.size} != wordCount $wordCount"
    }

    var words by remember { mutableStateOf(initialWords) }

    Column(
        modifier = modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        // Lay out 2 words per row so the user sees the index alongside
        // each field; matches the desktop / iOS wallet UX.
        words.chunked(2).forEachIndexed { rowIdx, pair ->
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                pair.forEachIndexed { colIdx, word ->
                    val absoluteIdx = rowIdx * 2 + colIdx
                    OutlinedTextField(
                        value = word,
                        onValueChange = { newWord ->
                            val sanitised = newWord.lowercase().trim()
                            val updated = words.toMutableList().apply {
                                set(absoluteIdx, sanitised)
                            }
                            words = updated
                            onPhraseChange(updated.joinToString(" ").trim())
                        },
                        label = { Text("${absoluteIdx + 1}") },
                        singleLine = true,
                        keyboardOptions = KeyboardOptions(
                            capitalization = KeyboardCapitalization.None,
                            autoCorrect = false,
                            keyboardType = KeyboardType.Text,
                            imeAction = if (absoluteIdx == wordCount - 1) {
                                ImeAction.Done
                            } else {
                                ImeAction.Next
                            },
                        ),
                        modifier = Modifier
                            .weight(1f)
                            .padding(vertical = 4.dp),
                    )
                }
            }
        }
    }
}
