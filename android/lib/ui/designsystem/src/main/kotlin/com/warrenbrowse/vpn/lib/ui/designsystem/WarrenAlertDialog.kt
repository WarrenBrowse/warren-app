package com.warrenbrowse.vpn.lib.ui.designsystem

import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.rememberTransition
import androidx.compose.animation.core.tween
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.AlertDialogDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.DialogWindowProvider

// Matches the desktop DialogPopup: 250ms, from 0.9 scale and full transparency,
// with the scrim fading on the same clock.
private const val DIALOG_TRANSITION_MILLIS = 250
private const val DIALOG_INITIAL_SCALE = 0.9f
private const val DIALOG_SCRIM_DIM = 0.32f

/**
 * The app's alert dialog: a Material3 [AlertDialog] that scales and fades in
 * instead of hard-cutting, with its scrim fading on the same clock.
 *
 * Same parameters as [AlertDialog], so migrating a call site is a rename. The
 * exit runs for a dismissal this dialog is asked to perform
 * ([onDismissRequest]); a caller that removes the dialog by flipping its own
 * state takes it out of composition directly, and nothing can animate a
 * composable that is already gone.
 */
@Composable
fun WarrenAlertDialog(
    onDismissRequest: () -> Unit,
    confirmButton: @Composable () -> Unit,
    modifier: Modifier = Modifier,
    dismissButton: (@Composable () -> Unit)? = null,
    icon: (@Composable () -> Unit)? = null,
    title: (@Composable () -> Unit)? = null,
    text: (@Composable () -> Unit)? = null,
    containerColor: Color = AlertDialogDefaults.containerColor,
    properties: DialogProperties = DialogProperties(),
) {
    val visibleState = remember { MutableTransitionState(false) }
    LaunchedEffect(Unit) { visibleState.targetState = true }

    // The exit has to finish before the caller is told, otherwise the dialog
    // window closes on the first frame and there is nothing left to animate.
    LaunchedEffect(visibleState.currentState, visibleState.isIdle) {
        if (visibleState.isIdle && !visibleState.currentState && !visibleState.targetState) {
            onDismissRequest()
        }
    }

    val transition = rememberTransition(visibleState, label = "warrenAlertDialog")
    val alpha by
        transition.animateFloat(
            transitionSpec = { tween(DIALOG_TRANSITION_MILLIS) },
            label = "alpha",
        ) { visible ->
            if (visible) 1f else 0f
        }
    val scale by
        transition.animateFloat(
            transitionSpec = { tween(DIALOG_TRANSITION_MILLIS) },
            label = "scale",
        ) { visible ->
            if (visible) 1f else DIALOG_INITIAL_SCALE
        }

    AlertDialog(
        onDismissRequest = { visibleState.targetState = false },
        confirmButton = {
            // Rendered inside the dialog's own window, which is the only place
            // the window handle is reachable from.
            AnimatedScrim(alpha)
            confirmButton()
        },
        dismissButton = dismissButton,
        icon = icon,
        title = title,
        text = text,
        containerColor = containerColor,
        properties = properties,
        modifier = modifier.scale(scale).alpha(alpha),
    )
}

/**
 * Fades the platform-owned scrim with the card. Without it the dim appears in
 * one frame under a card that is still scaling in.
 */
@Composable
private fun AnimatedScrim(alpha: Float) {
    val dialogWindow = (LocalView.current.parent as? DialogWindowProvider)?.window
    SideEffect { dialogWindow?.setDimAmount(alpha * DIALOG_SCRIM_DIM) }
}
