package com.warrenbrowse.vpn.core.animation

import androidx.compose.animation.AnimatedContentTransitionScope
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.navigation3.runtime.metadata
import androidx.navigation3.ui.NavDisplay

/**
 * Forward slide for a linear wizard (privacy disclaimer, wallet creation, onboarding steps).
 *
 * Unlike [slideInHorizontalTransition], where the current screen stays put and the pushed one
 * stacks over it, both steps move: the outgoing one recedes while the next arrives full width.
 * That reads as advancing along one path instead of descending into a sub-page, and it is the
 * desktop push transition (new from 100%, old to -33%).
 *
 * @param crossfadeOnEnter true when this step is being reached by a root swap (from the splash
 *   screen), where there is no spatial relationship to express and a slide would be a lie.
 */
fun wizardForwardTransition(crossfadeOnEnter: () -> Boolean = { false }): Map<String, Any> =
    metadata {
        put(NavDisplay.TransitionKey) {
            if (crossfadeOnEnter()) {
                fadeIn(tween(TRANSITION_DEFAULT_DURATION_MS)) togetherWith
                    fadeOut(tween(TRANSITION_DEFAULT_DURATION_MS))
            } else {
                fadeIn(tween(TRANSITION_DEFAULT_DURATION_MS)) +
                    slideIntoContainer(
                        animationSpec = tween(TRANSITION_DEFAULT_DURATION_MS),
                        towards = AnimatedContentTransitionScope.SlideDirection.Start,
                        initialOffset = { it },
                    ) togetherWith
                    slideOutOfContainer(
                        animationSpec = tween(TRANSITION_DEFAULT_DURATION_MS),
                        towards = AnimatedContentTransitionScope.SlideDirection.Start,
                        targetOffset = { (it * RECEDE_SLIDE_FACTOR).toInt() },
                    ) + fadeOut(tween(TRANSITION_DEFAULT_DURATION_MS))
            }
        }

        // Exact mirror: the previous step comes back from where it receded to.
        put(NavDisplay.PopTransitionKey) { wizardBackTransform() }

        put(NavDisplay.PredictivePopTransitionKey) { wizardBackTransform() }
    }

private fun AnimatedContentTransitionScope<*>.wizardBackTransform() =
    fadeIn(tween(TRANSITION_DEFAULT_DURATION_MS)) +
        slideIntoContainer(
            animationSpec = tween(TRANSITION_DEFAULT_DURATION_MS),
            towards = AnimatedContentTransitionScope.SlideDirection.End,
            initialOffset = { (it * RECEDE_SLIDE_FACTOR).toInt() },
        ) togetherWith
        slideOutOfContainer(
            animationSpec = tween(TRANSITION_DEFAULT_DURATION_MS),
            towards = AnimatedContentTransitionScope.SlideDirection.End,
            targetOffset = { it },
        ) + fadeOut(tween(TRANSITION_DEFAULT_DURATION_MS))
