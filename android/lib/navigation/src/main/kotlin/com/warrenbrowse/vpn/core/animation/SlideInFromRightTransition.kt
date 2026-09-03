package com.warrenbrowse.vpn.core.animation

import androidx.compose.animation.AnimatedContentTransitionScope
import androidx.compose.animation.ContentTransform
import androidx.compose.animation.core.tween
import androidx.compose.animation.togetherWith
import androidx.navigation3.runtime.metadata
import androidx.navigation3.ui.NavDisplay

/**
 * The desktop push: the pushed screen arrives from the full width while the
 * one underneath recedes a third of the way out, so the stack reads as depth
 * rather than as a curtain sliding over a static page. No fade: the movement
 * carries the whole transition, as it does on desktop.
 */
fun AnimatedContentTransitionScope<*>.screenPushTransform(): ContentTransform =
    slideIntoContainer(
        animationSpec = tween(TRANSITION_DEFAULT_DURATION_MS),
        towards = AnimatedContentTransitionScope.SlideDirection.Start,
        initialOffset = { (it * ENTER_TRANSITION_SLIDE_FACTOR).toInt() },
    ) togetherWith
        slideOutOfContainer(
            animationSpec = tween(TRANSITION_DEFAULT_DURATION_MS),
            towards = AnimatedContentTransitionScope.SlideDirection.Start,
            targetOffset = { (it * RECEDE_SLIDE_FACTOR).toInt() },
        )

/** Exact mirror of [screenPushTransform]: the revealed screen comes back from where it receded to. */
fun AnimatedContentTransitionScope<*>.screenPopTransform(): ContentTransform =
    slideIntoContainer(
        animationSpec = tween(TRANSITION_DEFAULT_DURATION_MS),
        towards = AnimatedContentTransitionScope.SlideDirection.End,
        initialOffset = { (it * RECEDE_SLIDE_FACTOR).toInt() },
    ) togetherWith
        slideOutOfContainer(
            animationSpec = tween(TRANSITION_DEFAULT_DURATION_MS),
            towards = AnimatedContentTransitionScope.SlideDirection.End,
            targetOffset = { (it * ENTER_TRANSITION_SLIDE_FACTOR).toInt() },
        )

fun slideInHorizontalTransition(): Map<String, Any> = metadata {
    put(NavDisplay.TransitionKey) { screenPushTransform() }

    put(NavDisplay.PopTransitionKey) { screenPopTransform() }

    // The predictive back gesture previews the same movement the pop finishes.
    put(NavDisplay.PredictivePopTransitionKey) { screenPopTransform() }
}
