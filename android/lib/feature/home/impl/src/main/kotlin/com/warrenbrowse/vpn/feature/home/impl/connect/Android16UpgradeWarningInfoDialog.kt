package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.tooling.preview.Preview
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.common.compose.clickableAnnotatedString
import com.warrenbrowse.vpn.common.compose.createUriHook
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.lib.ui.component.dialog.InfoDialog
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme

@Preview
@Composable
private fun PreviewAndroid16UpgradeWarningInfoDialog() {
    AppTheme { Android16UpgradeWarningInfoDialog(onDismiss = {}, onClickForum = {}) }
}

@Composable
fun Android16UpgradeWarningInfo(navigator: Navigator) {
    // The forum is the only support door left, so the link has to open it
    // rather than copy an address the user then has to do something with.
    val forumUrl = stringResource(R.string.community_forum_url)
    val openForum = LocalUriHandler.current.createUriHook(forumUrl)
    Android16UpgradeWarningInfoDialog(
        onDismiss = dropUnlessResumed { navigator.goBack() },
        onClickForum = { openForum() },
    )
}

@Composable
fun Android16UpgradeWarningInfoDialog(onDismiss: () -> Unit, onClickForum: (String) -> Unit) {
    InfoDialog(
        title = stringResource(id = R.string.android_16_upgrade_warning_title),
        message = stringResource(id = R.string.android_16_upgrade_warning_dialog_first_message),
        additionalInfo =
            clickableAnnotatedString(
                text = stringResource(R.string.android_16_upgrade_warning_dialog_second_message),
                linkStyle =
                    SpanStyle(
                        color = MaterialTheme.colorScheme.onSurface,
                        textDecoration = TextDecoration.Underline,
                    ),
                argument = stringResource(R.string.community_forum_url),
                onClick = onClickForum,
            ),
        showIcon = false,
        onDismiss = onDismiss,
    )
}
