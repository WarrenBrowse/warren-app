package com.warrenbrowse.vpn.lib.ui.component

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.OpenInNew
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens

/**
 * Link to the public help page, shown under every login and onboarding error
 * (desktop `OnboardingForumHint`). The page is the triage door for a user who
 * has no account yet: forum login is gated on having paid, so an error at this
 * stage would otherwise be a dead end.
 */
@Composable
fun WarrenHelpLink(modifier: Modifier = Modifier) {
    val helpUrl = stringResource(R.string.help_page_url)
    Row(modifier = modifier, verticalAlignment = Alignment.CenterVertically) {
        Text(
            text =
                buildAnnotatedString {
                    withLink(LinkAnnotation.Url(helpUrl)) {
                        withStyle(
                            style =
                                SpanStyle(
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    textDecoration = TextDecoration.Underline,
                                )
                        ) {
                            append(stringResource(R.string.help_page_link))
                        }
                    }
                },
            style = MaterialTheme.typography.labelLarge,
            modifier = Modifier.padding(end = Dimens.miniPadding),
        )
        Icon(
            imageVector = Icons.AutoMirrored.Rounded.OpenInNew,
            contentDescription = stringResource(R.string.external_link),
            modifier = Modifier.size(Dimens.privacyPolicyIconSize),
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
