package com.warrenbrowse.vpn.lib.ui.component

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.warrenbrowse.vpn.lib.model.forum.unreadLabel
import com.warrenbrowse.vpn.lib.ui.tag.TOP_BAR_FORUM_BUTTON_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.brand

/**
 * What the header's forum slot carries (desktop `AppMainHeaderForumButton`):
 * the activity bell with its unread count, or the lifebuoy into the forum for
 * a wallet with no forum account yet. Absent when the setting is off.
 *
 * Immutable so the header compares slots by value: the Connect screen builds
 * one on every recomposition, and an unstable parameter would recompose the
 * whole top bar on every tunnel edge although nothing in it changed.
 */
@Immutable
sealed interface ForumHeaderSlot {
    data class Activity(val unread: Int) : ForumHeaderSlot

    data object Community : ForumHeaderSlot
}

/**
 * The bell with its count badge, or the lifebuoy. Same outline weight as the
 * account and settings glyphs beside it, so the header keeps one stroke across
 * its three buttons.
 */
@Composable
fun ForumHeaderButton(
    slot: ForumHeaderSlot,
    tint: Color,
    enabled: Boolean,
    onClick: () -> Unit,
) {
    val label =
        when (slot) {
            is ForumHeaderSlot.Activity ->
                if (slot.unread > 0) {
                    stringResource(R.string.forum_activity_header_unread, unreadLabel(slot.unread))
                } else {
                    stringResource(R.string.forum_activity_title)
                }
            ForumHeaderSlot.Community ->
                stringResource(R.string.community_forum) + ". " + stringResource(R.string.opens_externally)
        }
    IconButton(
        modifier =
            Modifier.testTag(TOP_BAR_FORUM_BUTTON_TEST_TAG).semantics { contentDescription = label },
        enabled = enabled,
        onClick = onClick,
    ) {
        Box {
            Icon(
                painter =
                    painterResource(
                        id =
                            when (slot) {
                                is ForumHeaderSlot.Activity -> R.drawable.ic_bell_outline
                                ForumHeaderSlot.Community -> R.drawable.ic_lifebuoy_outline
                            }
                    ),
                tint = tint,
                contentDescription = null,
                modifier = Modifier.size(Dimens.topBarActionIconSize),
            )
            if (slot is ForumHeaderSlot.Activity && slot.unread > 0) {
                UnreadCountBadge(unread = slot.unread, modifier = Modifier.align(Alignment.TopEnd))
            }
        }
    }
}

/**
 * The brand ocre on purpose, none of the three state accents: red is
 * disconnected, green is connected and orange is connecting, so any of them
 * here would read as a change in the tunnel rather than as forum activity.
 * Dark text on it, since ocre is a mid-light tone.
 */
@Composable
private fun UnreadCountBadge(unread: Int, modifier: Modifier = Modifier) {
    Text(
        text = unreadLabel(unread),
        color = Color.Black,
        fontSize = 10.sp,
        lineHeight = 15.sp,
        fontWeight = FontWeight.SemiBold,
        textAlign = TextAlign.Center,
        maxLines = 1,
        modifier =
            modifier
                .offset(x = 4.dp, y = (-2).dp)
                .background(MaterialTheme.colorScheme.brand, CircleShape)
                .defaultMinSize(minWidth = 15.dp, minHeight = 15.dp)
                .padding(horizontal = 4.dp),
    )
}
