package com.warrenbrowse.vpn.feature.login.impl

import kotlin.math.floor
import kotlin.random.Random

// How many words of a freshly shown recovery phrase the user types back before the account is
// used. Three out of twelve is what a person who wrote the phrase down answers in seconds and
// someone who only ticked a box cannot.
const val BACKUP_CHECK_WORD_COUNT = 3

/** Draws the zero-based positions of the words to ask for, distinct and in phrase order. */
fun pickBackupCheckPositions(
    wordCount: Int,
    random: () -> Double = { Random.nextDouble() },
): List<Int> {
    val wanted = minOf(BACKUP_CHECK_WORD_COUNT, wordCount)
    val positions = mutableSetOf<Int>()
    while (positions.size < wanted) {
        positions.add(minOf(wordCount - 1, floor(random() * wordCount).toInt()))
    }
    return positions.sorted()
}

fun backupCheckPasses(phrase: String, positions: List<Int>, answers: List<String>): Boolean {
    val words = phrase.trim().lowercase().split(Regex("""\s+"""))
    return positions.isNotEmpty() &&
        positions.size == answers.size &&
        positions.zip(answers).all { (position, answer) ->
            val given = answer.trim().lowercase()
            given.isNotEmpty() && given == words.getOrNull(position)
        }
}
