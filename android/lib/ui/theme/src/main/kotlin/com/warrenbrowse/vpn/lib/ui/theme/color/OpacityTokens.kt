package com.warrenbrowse.vpn.lib.ui.theme.color

import com.warrenbrowse.vpn.lib.ui.theme.tokens.DesignTokens

/**
 * The desktop neutral surface and text/icon ladders (`color-tokens.ts`
 * `blue10..80`, `whiteOnDarkBlue60`) under the Android names that predate the
 * generated tokens. Hueless, so the charcoal theme never casts a tint.
 */
internal object OpacityTokens {
    // White-on-charcoal text/icon grey.
    val WhiteOnDarkBlue60 = DesignTokens.Colors.WhiteOnDarkBlue60

    // Neutral grey surface ladder (darkest -> lightest).
    val BlueOnDarkBlue10 = DesignTokens.Colors.Blue10
    val BlueOnDarkBlue20 = DesignTokens.Colors.Blue20
    val BlueOnDarkBlue40 = DesignTokens.Colors.Blue40
    val BlueOnDarkBlue50 = DesignTokens.Colors.Blue50
    val BlueOnDarkBlue60 = DesignTokens.Colors.Blue60
    val BlueOnDarkBlue80 = DesignTokens.Colors.Blue80
}
