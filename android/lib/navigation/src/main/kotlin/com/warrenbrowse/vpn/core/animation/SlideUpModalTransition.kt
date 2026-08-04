package com.warrenbrowse.vpn.core.animation

import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.navigation3.runtime.metadata
import androidx.navigation3.ui.NavDisplay

/**
 * Vertical show/dismiss pair for surfaces that are a temporary overlay rather than a level deeper
 * in the hierarchy (select location, settings, anything opened with `isModal`).
 *
 * The Y axis is what tells the user "this sits on top and will drop back down"; the horizontal
 * slide stays reserved for going deeper. Same split as the desktop show/dismiss transition types.
 */
fun slideUpModalTransition(): Map<String, Any> = metadata {

    // Raise the pushed screen over the current one, which stays put underneath.
    put(NavDisplay.TransitionKey) {
        slideInVertically(
            animationSpec = tween(TRANSITION_DEFAULT_DURATION_MS),
            initialOffsetY = { it },
        ) + fadeIn(tween(TRANSITION_DEFAULT_DURATION_MS)) togetherWith ExitTransition.None
    }

    // Drop the dismissed screen back down, revealing what was underneath.
    put(NavDisplay.PopTransitionKey) { modalDismissTransform() }

    put(NavDisplay.PredictivePopTransitionKey) { modalDismissTransform() }
}

private fun modalDismissTransform() =
    EnterTransition.None togetherWith
        slideOutVertically(
            animationSpec = tween(TRANSITION_DEFAULT_DURATION_MS),
            targetOffsetY = { it },
        ) + fadeOut(tween(TRANSITION_DEFAULT_DURATION_MS))
