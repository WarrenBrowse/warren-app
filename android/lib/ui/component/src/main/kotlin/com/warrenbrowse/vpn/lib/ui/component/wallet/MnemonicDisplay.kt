package com.warrenbrowse.vpn.lib.ui.component.wallet

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.isSensitiveData
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.lib.ui.resource.R

/**
 * Read-only display of a BIP39 mnemonic.
 *
 * By default the phrase is blurred and the user taps to reveal (defeats
 * clipboard-scraping/over-the-shoulder leaks). Pass [alwaysRevealed] = true to
 * show it unblurred from the start (the backup screen, which mirrors the
 * desktop where the phrase is always visible alongside a copy action).
 *
 * The mnemonic is rendered as a 6x2 grid of word cards (or 8x3 for the 24-word
 * variant) matching `MnemonicInput`'s layout so the user can visually correlate
 * the backup view with the input field.
 *
 * Caller responsibilities:
 *   - The `phrase` parameter is the cleartext mnemonic and must only be
 *     supplied just-in-time after `WalletRepository.unlock()`. Once this
 *     composable exits scope the reference goes away.
 *   - Wrap a navigation-onBack handler that re-locks the wallet (set
 *     `WalletState` back to `Locked`) so the mnemonic does not leak into
 *     a paused Activity.
 */
@Composable
fun MnemonicDisplay(
    phrase: String,
    modifier: Modifier = Modifier,
    alwaysRevealed: Boolean = false,
) {
    var revealed by remember { mutableStateOf(alwaysRevealed) }
    val words = remember(phrase, revealed) { mnemonicWordsShown(phrase, revealed) }

    Column(
        modifier = modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        if (!alwaysRevealed) {
            Text(
                text = stringResource(
                    if (revealed) R.string.wallet_mnemonic_hide_hint
                    else R.string.wallet_mnemonic_reveal_hint
                ),
                style = MaterialTheme.typography.bodyMedium,
                textAlign = TextAlign.Center,
                modifier = Modifier.fillMaxWidth(),
            )
        }
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .then(if (alwaysRevealed) Modifier else Modifier.clickable { revealed = !revealed }),
        ) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    // Keep the recovery words out of the accessibility tree and
                    // screenshots taken by services that honor sensitivity.
                    .semantics { isSensitiveData = true }
                    .then(if (revealed) Modifier else Modifier.blur(BLUR_RADIUS_DP.dp)),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                words.chunked(2).forEachIndexed { rowIdx, pair ->
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        pair.forEachIndexed { colIdx, word ->
                            val idx = rowIdx * 2 + colIdx
                            Surface(
                                modifier = Modifier
                                    .weight(1f)
                                    .padding(vertical = 4.dp),
                                shape = RoundedCornerShape(WORD_CORNER_DP.dp),
                                color = MaterialTheme.colorScheme.surfaceVariant,
                            ) {
                                Row(
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .background(MaterialTheme.colorScheme.surfaceVariant)
                                        .padding(WORD_PADDING_DP.dp),
                                    horizontalArrangement = Arrangement.SpaceBetween,
                                    verticalAlignment = Alignment.CenterVertically,
                                ) {
                                    Text(
                                        text = "${idx + 1}.",
                                        style = MaterialTheme.typography.labelMedium,
                                    )
                                    Text(
                                        text = word,
                                        style = MaterialTheme.typography.bodyMedium,
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/**
 * The words the grid draws: the phrase once revealed, the same placeholder in every cell before.
 * The blur alone is not enough, since Compose ignores it before Android 12 and the words would
 * then be readable without a tap.
 */
internal fun mnemonicWordsShown(phrase: String, revealed: Boolean): List<String> {
    val words = phrase.trim().split(' ')
    return if (revealed) words else List(words.size) { HIDDEN_WORD }
}

private const val HIDDEN_WORD = "••••••"
private const val BLUR_RADIUS_DP = 12
private const val WORD_CORNER_DP = 8
private const val WORD_PADDING_DP = 12
