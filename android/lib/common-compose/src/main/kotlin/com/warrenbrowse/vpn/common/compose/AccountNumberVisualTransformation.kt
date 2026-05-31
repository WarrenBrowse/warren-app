package com.warrenbrowse.vpn.common.compose

import androidx.compose.foundation.text.input.OutputTransformation
import androidx.compose.foundation.text.input.insert
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.input.OffsetMapping
import androidx.compose.ui.text.input.TransformedText
import androidx.compose.ui.text.input.VisualTransformation
import kotlin.math.max

const val ACCOUNT_NUMBER_SEPARATOR = " "
const val ACCOUNT_NUMBER_CHUNK_SIZE = 4
const val PASSWORD_UNICODE = '●'

/**
 * Identity transformation for single-token fields such as the Warren
 * wallet SS58 address. Unlike [accountNumberVisualTransformation], it
 * does NOT chunk the input into groups of [ACCOUNT_NUMBER_CHUNK_SIZE]:
 * a Warren address (`wb…`) is a single base58 token and must render
 * verbatim, with a 1:1 offset mapping so cursor positions stay correct.
 *
 * The chunking [accountNumberVisualTransformation] /
 * [accountNumberOutputTransformation] below remain for the legacy
 * Mullvad account-number login field (16-digit, grouped in 4s).
 */
fun walletAddressVisualTransformation() =
    VisualTransformation { text ->
        TransformedText(AnnotatedString(text.text), OffsetMapping.Identity)
    }

fun accountNumberVisualTransformation(showAccount: Boolean = true, showLastX: Int = 0) =
    VisualTransformation {
        val replacementLength = max(0, it.length - showLastX)
        val inputString =
            if (showAccount) it
            else {
                it.replaceRange(
                    0..<replacementLength,
                    PASSWORD_UNICODE.toString().repeat(replacementLength),
                )
            }
        val transformedString =
            inputString.chunked(ACCOUNT_NUMBER_CHUNK_SIZE).joinToString(ACCOUNT_NUMBER_SEPARATOR)
        val transformedAnnotatedString = AnnotatedString(transformedString)

        TransformedText(
            transformedAnnotatedString,
            object : OffsetMapping {
                override fun originalToTransformed(offset: Int): Int =
                    offset + (offset - 1) / ACCOUNT_NUMBER_CHUNK_SIZE

                override fun transformedToOriginal(offset: Int): Int =
                    offset - (offset - 1) / (ACCOUNT_NUMBER_CHUNK_SIZE + 1)
            },
        )
    }

fun accountNumberOutputTransformation(showAccount: Boolean = true, showLastX: Int = 0) =
    OutputTransformation {
        // Insert separators between chunks (from right to left to maintain correct positions)
        // Start from the last chunk boundary that is within the string (not at the end)
        var position = ((length - 1) / ACCOUNT_NUMBER_CHUNK_SIZE) * ACCOUNT_NUMBER_CHUNK_SIZE
        while (position > 0) {
            insert(position, ACCOUNT_NUMBER_SEPARATOR)
            position -= ACCOUNT_NUMBER_CHUNK_SIZE
        }

        if (showAccount) return@OutputTransformation

        val length = length
        val visibleStart = (length - showLastX).coerceAtLeast(0)

        for (i in 0 until visibleStart) {
            val c = charAt(i)
            if (c.toString() != ACCOUNT_NUMBER_SEPARATOR) {
                replace(i, i + 1, PASSWORD_UNICODE.toString())
            }
        }
    }
