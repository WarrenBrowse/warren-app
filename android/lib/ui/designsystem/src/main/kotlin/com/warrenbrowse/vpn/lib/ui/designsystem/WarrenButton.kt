package com.warrenbrowse.vpn.lib.ui.designsystem

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonColors
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha20
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha40
import com.warrenbrowse.vpn.lib.ui.theme.color.AlphaInvisible
import com.warrenbrowse.vpn.lib.ui.theme.color.errorDisabled
import com.warrenbrowse.vpn.lib.ui.theme.color.positive
import com.warrenbrowse.vpn.lib.ui.theme.color.primaryDisabled
import com.warrenbrowse.vpn.lib.ui.theme.color.tertiaryDisabled

// Warren buttons use a 4dp corner radius to match the desktop (Electron) app,
// instead of the Material3 default fully-rounded pill shape.
private val WarrenButtonShape = RoundedCornerShape(4.dp)

@Preview
@Composable
private fun PreviewNegativeButtonEnabled() {
    AppTheme { NegativeButton(onClick = {}, text = "Negative Button") }
}

@Preview
@Composable
private fun PreviewNegativeButtonDisabled() {
    AppTheme { NegativeButton(onClick = {}, text = "Negative Button", isEnabled = false) }
}

@Preview
@Composable
private fun PreviewVariantButtonEnabled() {
    AppTheme { VariantButton(onClick = {}, text = "Variant Button") }
}

@Preview
@Composable
private fun PreviewVariantButtonDisabled() {
    AppTheme { VariantButton(onClick = {}, text = "Variant Button", isEnabled = false) }
}

@Preview
@Composable
private fun PreviewPrimaryButtonEnabled() {
    AppTheme { PrimaryButton(onClick = {}, text = "Primary Button") }
}

@Preview
@Composable
private fun PreviewPrimaryButtonDisabled() {
    AppTheme { PrimaryButton(onClick = {}, text = "Primary Button", isEnabled = false) }
}

@Preview
@Composable
private fun PreviewTextButtonEnabled() {
    AppTheme { PrimaryTextButton(onClick = {}, text = "Text Button") }
}

@Preview
@Composable
private fun PreviewTextButtonDisabled() {
    AppTheme { PrimaryTextButton(onClick = {}, text = "Text Button", isEnabled = false) }
}

@Composable
fun NegativeButton(
    onClick: () -> Unit,
    text: String,
    modifier: Modifier = Modifier,
    colors: ButtonColors =
        ButtonDefaults.buttonColors(
            containerColor = MaterialTheme.colorScheme.error,
            contentColor = MaterialTheme.colorScheme.onError,
            disabledContentColor = MaterialTheme.colorScheme.onError.copy(alpha = Alpha20),
            disabledContainerColor = MaterialTheme.colorScheme.errorDisabled,
        ),
    isEnabled: Boolean = true,
    isLoading: Boolean = false,
    content: @Composable (() -> Unit)? = null,
) {
    BaseButton(
        onClick = onClick,
        colors = colors,
        text = text,
        modifier = modifier,
        isEnabled = isEnabled,
        isLoading = isLoading,
        trailingIcon = content,
    )
}

@Suppress("ComposableLambdaParameterNaming")
@Composable
fun VariantButton(
    onClick: () -> Unit,
    text: String,
    modifier: Modifier = Modifier,
    background: Color = MaterialTheme.colorScheme.positive,
    colors: ButtonColors =
        ButtonDefaults.buttonColors(
            containerColor = background,
            contentColor = MaterialTheme.colorScheme.onTertiary,
            disabledContentColor = MaterialTheme.colorScheme.onTertiary.copy(alpha = Alpha20),
            disabledContainerColor = MaterialTheme.colorScheme.tertiaryDisabled,
        ),
    isEnabled: Boolean = true,
    isLoading: Boolean = false,
    icon: @Composable (() -> Unit)? = null,
) {
    BaseButton(
        onClick = onClick,
        colors = colors,
        text = text,
        modifier = modifier,
        isEnabled = isEnabled,
        isLoading = isLoading,
        trailingIcon = icon,
    )
}

@Composable
fun PrimaryButton(
    onClick: () -> Unit,
    text: String,
    modifier: Modifier = Modifier,
    colors: ButtonColors =
        ButtonDefaults.buttonColors(
            containerColor = MaterialTheme.colorScheme.primary,
            contentColor = MaterialTheme.colorScheme.onPrimary,
            disabledContentColor = MaterialTheme.colorScheme.onPrimary.copy(alpha = Alpha20),
            disabledContainerColor = MaterialTheme.colorScheme.primaryDisabled,
        ),
    isEnabled: Boolean = true,
    isLoading: Boolean = false,
    leadingIcon: @Composable (() -> Unit)? = null,
    trailingIcon: @Composable (() -> Unit)? = null,
    label: (@Composable RowScope.() -> Unit)? = null,
) {
    BaseButton(
        onClick = onClick,
        colors = colors,
        text = text,
        modifier = modifier,
        isEnabled = isEnabled,
        isLoading = isLoading,
        leadingIcon = leadingIcon,
        trailingIcon = trailingIcon,
        label = label,
    )
}

@Composable
fun PrimaryTextButton(
    onClick: () -> Unit,
    text: String,
    modifier: Modifier = Modifier,
    colors: ButtonColors =
        ButtonDefaults.textButtonColors(
            contentColor = MaterialTheme.colorScheme.onPrimary,
            disabledContentColor = MaterialTheme.colorScheme.onPrimary.copy(alpha = Alpha20),
        ),
    textDecoration: TextDecoration = TextDecoration.None,
    isEnabled: Boolean = true,
    isLoading: Boolean = false,
    leadingIcon: @Composable (() -> Unit)? = null,
    trailingIcon: @Composable (() -> Unit)? = null,
) {
    val hasIcon = leadingIcon != null || trailingIcon != null
    TextButton(
        onClick = onClick,
        modifier = modifier.wrapContentHeight().width(IntrinsicSize.Max),
        colors = colors,
        shape = WarrenButtonShape,
        // A spinner that stays tappable lets the user fire the same in-flight
        // action twice; loading implies disabled, as on NegativeOutlinedButton.
        enabled = !isLoading && isEnabled,
        contentPadding =
            if (hasIcon) {
                PaddingValues(vertical = Dimens.buttonSpacing)
            } else {
                ButtonDefaults.TextButtonContentPadding
            },
    ) {
        BaseButtonContent(
            text = text,
            textDecoration = textDecoration,
            isLoading = isLoading,
            leadingIcon = leadingIcon,
            trailingIcon = trailingIcon,
        )
    }
}

@Composable
fun NegativeOutlinedButton(
    onClick: () -> Unit,
    text: String,
    modifier: Modifier = Modifier,
    colors: ButtonColors =
        ButtonDefaults.outlinedButtonColors(
            contentColor = MaterialTheme.colorScheme.onError,
            disabledContentColor = MaterialTheme.colorScheme.onError.copy(alpha = Alpha20),
        ),
    border: BorderStroke =
        BorderStroke(
            width = Dimens.outLineButtonBorderWidth,
            color = MaterialTheme.colorScheme.error,
        ),
    isEnabled: Boolean = true,
    isLoading: Boolean = false,
    leadingIcon: @Composable (() -> Unit)? = null,
    trailingIcon: @Composable (() -> Unit)? = null,
) {
    val hasIcon = leadingIcon != null || trailingIcon != null
    OutlinedButton(
        onClick = onClick,
        modifier = modifier.wrapContentHeight().width(IntrinsicSize.Max),
        colors = colors,
        shape = WarrenButtonShape,
        enabled = !isLoading && isEnabled,
        border = border,
        contentPadding =
            if (hasIcon) {
                PaddingValues(vertical = Dimens.buttonSpacing)
            } else {
                ButtonDefaults.TextButtonContentPadding
            },
    ) {
        BaseButtonContent(
            text = text,
            isLoading = isLoading,
            leadingIcon = leadingIcon,
            trailingIcon = trailingIcon,
        )
    }
}

@Suppress("ComposableLambdaParameterNaming")
@Composable
private fun BaseButton(
    onClick: () -> Unit,
    colors: ButtonColors,
    text: String,
    modifier: Modifier = Modifier,
    isEnabled: Boolean = true,
    isLoading: Boolean = false,
    leadingIcon: @Composable (() -> Unit)? = null,
    trailingIcon: @Composable (() -> Unit)? = null,
    // Replaces the plain label when the caller needs to animate it. Whatever it
    // renders carries the button's semantics, so it must show [text].
    label: (@Composable RowScope.() -> Unit)? = null,
) {
    val hasIcon = leadingIcon != null || trailingIcon != null
    Button(
        onClick = onClick,
        colors = colors,
        shape = WarrenButtonShape,
        // A spinner that stays tappable lets the user fire the same in-flight
        // action twice; loading implies disabled, as on NegativeOutlinedButton.
        enabled = !isLoading && isEnabled,
        contentPadding =
            if (hasIcon) {
                PaddingValues(vertical = Dimens.buttonSpacing)
            } else {
                ButtonDefaults.ContentPadding
            },
        modifier = modifier.wrapContentHeight().fillMaxWidth(),
    ) {
        BaseButtonContent(
            text = text,
            isLoading = isLoading,
            leadingIcon = leadingIcon,
            trailingIcon = trailingIcon,
            label = label,
        )
    }
}

@Composable
fun SmallPrimaryButton(
    onClick: () -> Unit,
    text: String,
    modifier: Modifier = Modifier,
    colors: ButtonColors =
        ButtonDefaults.buttonColors(
            containerColor = MaterialTheme.colorScheme.primary,
            contentColor = MaterialTheme.colorScheme.onPrimary,
            disabledContentColor = MaterialTheme.colorScheme.onPrimary.copy(alpha = Alpha20),
            disabledContainerColor = MaterialTheme.colorScheme.primaryDisabled,
        ),
) {
    Button(onClick = onClick, modifier = modifier, colors = colors, shape = WarrenButtonShape) {
        Text(text = text)
    }
}

@Composable
private fun RowScope.BaseButtonContent(
    text: String,
    isLoading: Boolean,
    textDecoration: TextDecoration = TextDecoration.None,
    leadingIcon: @Composable() (() -> Unit)?,
    trailingIcon: @Composable() (() -> Unit)?,
    label: (@Composable RowScope.() -> Unit)? = null,
) {
    when {
        leadingIcon != null ->
            Box(modifier = Modifier.padding(horizontal = Dimens.mediumPadding)) { leadingIcon() }

        trailingIcon != null ->
            // Used to center the text
            Box(
                modifier = Modifier.padding(horizontal = Dimens.mediumPadding).alpha(AlphaInvisible)
            ) {
                trailingIcon()
            }
    }
    if (isLoading) {
        WarrenCircularProgressIndicatorSmall()
    } else if (label != null) {
        label()
    } else {
        Text(
            text = text,
            textAlign = TextAlign.Center,
            style = MaterialTheme.typography.titleMedium,
            textDecoration = textDecoration,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
    }
    when {
        trailingIcon != null ->
            Box(modifier = Modifier.padding(horizontal = Dimens.mediumPadding)) { trailingIcon() }

        leadingIcon != null ->
            // Used to center the text
            Box(
                modifier = Modifier.padding(horizontal = Dimens.mediumPadding).alpha(AlphaInvisible)
            ) {
                leadingIcon()
            }
    }
}

/**
 * A text button readable on this palette. Material's `TextButton` colours its
 * label with `colorScheme.primary`, which here is the charcoal of the raised
 * cells (`#4A4846`): on a dialog or a card it is one shade off the surface and
 * the label vanishes (the forum consent prompt's Cancel did, literally). Every
 * text action uses this instead, with the surface's own foreground.
 */
@Composable
fun WarrenTextButton(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    colors: ButtonColors =
        ButtonDefaults.textButtonColors(
            contentColor = MaterialTheme.colorScheme.onSurface,
            disabledContentColor = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha40),
        ),
    contentPadding: PaddingValues = ButtonDefaults.TextButtonContentPadding,
    content: @Composable RowScope.() -> Unit,
) {
    TextButton(
        onClick = onClick,
        modifier = modifier,
        enabled = enabled,
        colors = colors,
        contentPadding = contentPadding,
        content = content,
    )
}
