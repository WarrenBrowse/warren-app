package com.warrenbrowse.vpn.feature.account.impl

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import com.warrenbrowse.vpn.lib.common.util.groupPasswordModeWithSpaces
import com.warrenbrowse.vpn.lib.common.util.groupWithSpaces

@Composable
fun AccountNumberView(
    accountNumber: String,
    obfuscateWithPasswordDots: Boolean,
    modifier: Modifier = Modifier,
) {
    InformationView(
        content =
            if (obfuscateWithPasswordDots) accountNumber.groupPasswordModeWithSpaces()
            else accountNumber.groupWithSpaces(),
        modifier = modifier,
        fontFamily = FontFamily.Monospace,
        whenMissing = MissingPolicy.SHOW_SPINNER,
    )
}
