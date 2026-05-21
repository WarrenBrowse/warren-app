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
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp

/**
 * Read-only display of a BIP39 mnemonic, blurred by default. The user
 * taps to reveal - per the D.5 brief Warren omits a "copy mnemonic" CTA
 * to defeat clipboard-scraping malware, so we make the reveal action
 * intentional and observable.
 *
 * The mnemonic is rendered as a 6x2 grid of word cards (or 8x3 for
 * 24-word variant) matching `MnemonicInput`'s layout so the user can
 * visually correlate the backup view with the input field.
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
) {
    val words = remember(phrase) { phrase.trim().split(' ') }
    var revealed by remember { mutableStateOf(false) }

    Column(
        modifier = modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(
            text = if (revealed) "Tap any word to hide" else "Tap to reveal",
            style = MaterialTheme.typography.bodyMedium,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth(),
        )
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .clickable { revealed = !revealed },
        ) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
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

private const val BLUR_RADIUS_DP = 12
private const val WORD_CORNER_DP = 8
private const val WORD_PADDING_DP = 12
