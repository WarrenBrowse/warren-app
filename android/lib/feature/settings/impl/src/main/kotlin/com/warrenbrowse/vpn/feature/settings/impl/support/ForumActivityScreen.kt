package com.warrenbrowse.vpn.feature.settings.impl.support

import android.content.Context
import android.text.format.DateUtils
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.common.compose.unlessIsDetail
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.lib.model.forum.ForumNotification
import com.warrenbrowse.vpn.lib.model.forum.ForumNotificationKind
import com.warrenbrowse.vpn.lib.model.forum.forumNotificationAgeIsRelative
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenCircularProgressIndicatorLarge
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha20
import com.warrenbrowse.vpn.lib.ui.theme.color.brand
import org.koin.androidx.compose.koinViewModel

/**
 * Community-forum activity, opened from the header bell (desktop
 * `ForumActivityView`). A full screen rather than a popover: Account and
 * Settings both open this way, and a dropdown over the connect screen exists
 * nowhere else in this app.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ForumActivity(navigator: Navigator) {
    val viewModel = koinViewModel<ForumActivityViewModel>()
    val state by viewModel.state.collectAsStateWithLifecycle()
    val handle by viewModel.handle.collectAsStateWithLifecycle()
    val uriHandler = LocalUriHandler.current
    val forumUrl = stringResource(R.string.community_forum_url)
    val ready = state as? ForumActivityUiState.Ready

    ScaffoldWithSmallTopBar(
        appBarTitle = stringResource(R.string.forum_activity_title),
        navigationIcon = {
            unlessIsDetail {
                NavigateBackIconButton(onNavigateBack = dropUnlessResumed { navigator.goBack() })
            }
        },
        actions = {
            if (ready?.hasUnread == true) {
                IconButton(onClick = viewModel::markAllRead) {
                    Icon(
                        imageVector = Icons.Rounded.Check,
                        contentDescription = stringResource(R.string.forum_activity_mark_all_read),
                    )
                }
            }
        },
    ) { modifier ->
        when (val current = state) {
            ForumActivityUiState.Loading ->
                Centered(modifier) { WarrenCircularProgressIndicatorLarge() }
            ForumActivityUiState.Error ->
                Centered(modifier) {
                    GlyphDisc(R.drawable.ic_forum_alert_circle)
                    Message(stringResource(R.string.forum_activity_error))
                    PrimaryButton(
                        text = stringResource(R.string.forum_activity_try_again),
                        onClick = viewModel::reload,
                    )
                }
            is ForumActivityUiState.Ready ->
                if (current.notifications.isEmpty()) {
                    Centered(modifier) {
                        GlyphDisc(R.drawable.ic_bell_outline)
                        Message(stringResource(R.string.forum_activity_empty))
                        PrimaryButton(
                            text = stringResource(R.string.forum_activity_open_forum),
                            onClick = { uriHandler.openUri(forumUrl) },
                        )
                        handle?.let {
                            Text(
                                text = stringResource(R.string.forum_activity_you_post_as, it),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                } else {
                    LazyColumn(
                        modifier = modifier,
                        contentPadding =
                            androidx.compose.foundation.layout.PaddingValues(
                                horizontal = Dimens.sideMarginNew,
                                vertical = Dimens.smallPadding,
                            ),
                        verticalArrangement = Arrangement.spacedBy(Dimens.tinyPadding),
                    ) {
                        items(current.notifications, key = { it.id }) { notification ->
                            ForumNotificationCard(
                                notification = notification,
                                onOpen = { path ->
                                    viewModel.markOneRead(notification.id)
                                    // The bare post, never `/session/sso`: that route
                                    // runs the whole wallet round trip on every visit.
                                    uriHandler.openUri(forumUrl.trimEnd('/') + path)
                                    // The reading happens in the browser now; a list
                                    // left up behind it is one the user has to
                                    // dismiss to get back to the app.
                                    navigator.goBack()
                                },
                            )
                        }
                    }
                }
        }
    }
}

@Composable
private fun Centered(modifier: Modifier, content: @Composable () -> Unit) {
    Column(
        modifier = modifier.fillMaxSize().padding(Dimens.sideMargin),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(Dimens.mediumPadding, Alignment.CenterVertically),
    ) {
        content()
    }
}

@Composable
private fun GlyphDisc(icon: Int) {
    Box(
        modifier =
            Modifier.size(64.dp)
                .background(MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha20), CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            painter = painterResource(icon),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(Dimens.mediumIconSize),
        )
    }
}

@Composable
private fun Message(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        textAlign = androidx.compose.ui.text.style.TextAlign.Center,
    )
}

/**
 * One notification: the kind glyph in its own disc so the eye can sort a reply
 * from a like without reading a word, the headline and age, the title and
 * excerpt clamped to two lines each. A row pointing at no post (a badge award)
 * is not clickable.
 */
@Composable
private fun ForumNotificationCard(notification: ForumNotification, onOpen: (String) -> Unit) {
    val context = LocalContext.current
    val headline = headlineFor(notification)
    val path = notification.path
    val opensExternally = stringResource(R.string.opens_externally)
    Row(
        modifier =
            Modifier.fillMaxWidth()
                .background(
                    MaterialTheme.colorScheme.primary.copy(alpha = if (notification.unread) 1f else 0.5f),
                    RoundedCornerShape(Dimens.dialogCornerRadius),
                )
                .then(
                    if (path != null) {
                        Modifier.clickable { onOpen(path) }
                            .semantics { contentDescription = "$headline. $opensExternally" }
                    } else {
                        Modifier.semantics { contentDescription = headline }
                    }
                )
                .padding(
                    start = Dimens.smallPadding,
                    top = Dimens.smallPadding,
                    end = Dimens.mediumPadding,
                    bottom = Dimens.smallPadding,
                ),
        horizontalArrangement = Arrangement.spacedBy(Dimens.smallPadding),
    ) {
        Box(
            modifier =
                Modifier.size(32.dp)
                    .background(
                        if (notification.unread) MaterialTheme.colorScheme.brand
                        else MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha20),
                        CircleShape,
                    ),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                painter = painterResource(iconFor(notification.kind)),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.background,
                modifier = Modifier.size(Dimens.smallIconSize),
            )
        }
        NotificationBody(notification, headline, relativeAge(context, notification.createdAt))
    }
}

/** The headline row with the unread dot and the age, then the title and the excerpt, each clamped. */
@Composable
private fun RowScope.NotificationBody(notification: ForumNotification, headline: String, age: String) {
    Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(Dimens.tinyPadding),
        ) {
            if (notification.unread) {
                Box(Modifier.size(7.dp).background(MaterialTheme.colorScheme.brand, CircleShape))
            }
            Text(
                text = headline,
                style = MaterialTheme.typography.bodyMedium,
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            Text(
                text = age,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        notification.title?.let {
            Text(text = it, style = MaterialTheme.typography.bodySmall, maxLines = 2, overflow = TextOverflow.Ellipsis)
        }
        notification.excerpt?.let {
            Text(
                text = it,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

/** Glyph that lets the eye sort a reply from a like before reading a word. */
private fun iconFor(kind: ForumNotificationKind): Int =
    when (kind) {
        ForumNotificationKind.LIKED -> R.drawable.ic_forum_heart_outline
        ForumNotificationKind.PRIVATE_MESSAGE -> R.drawable.ic_forum_message_outline
        ForumNotificationKind.MENTIONED,
        ForumNotificationKind.QUOTED -> R.drawable.ic_account_outline
        ForumNotificationKind.GRANTED_BADGE -> R.drawable.ic_forum_checkmark_circle
        ForumNotificationKind.ANNOUNCEMENT -> R.drawable.ic_forum_info_circle
        ForumNotificationKind.LINKED -> R.drawable.ic_forum_external
        ForumNotificationKind.REPLIED,
        ForumNotificationKind.POSTED,
        ForumNotificationKind.WATCHING_FIRST_POST -> R.drawable.ic_forum_reply_outline
        ForumNotificationKind.OTHER -> R.drawable.ic_bell_outline
    }

/** One line saying who did what, falling back when the forum said less. */
@Composable
private fun headlineFor(notification: ForumNotification): String {
    val actor = notification.actor ?: stringResource(R.string.forum_activity_someone)
    return when (notification.kind) {
        ForumNotificationKind.REPLIED,
        ForumNotificationKind.POSTED -> stringResource(R.string.forum_activity_replied, actor)
        ForumNotificationKind.LIKED -> stringResource(R.string.forum_activity_liked, actor)
        ForumNotificationKind.MENTIONED -> stringResource(R.string.forum_activity_mentioned, actor)
        ForumNotificationKind.QUOTED -> stringResource(R.string.forum_activity_quoted, actor)
        ForumNotificationKind.PRIVATE_MESSAGE -> stringResource(R.string.forum_activity_message, actor)
        ForumNotificationKind.LINKED -> stringResource(R.string.forum_activity_linked, actor)
        ForumNotificationKind.GRANTED_BADGE -> stringResource(R.string.forum_activity_badge)
        ForumNotificationKind.WATCHING_FIRST_POST -> stringResource(R.string.forum_activity_new_topic, actor)
        ForumNotificationKind.ANNOUNCEMENT -> stringResource(R.string.forum_activity_forum_updated)
        ForumNotificationKind.OTHER -> stringResource(R.string.forum_activity_generic)
    }
}

private const val MILLIS_PER_SECOND = 1000L

/**
 * Compact relative age ("2 h ago", "yesterday") while that is the useful thing
 * to say, an absolute date past a week. The platform formatter rather than
 * translated strings: it knows every locale's plural forms and wording, and it
 * never renders the past with a leading minus sign.
 */
private fun relativeAge(context: Context, createdAtSecs: Long): String {
    val now = System.currentTimeMillis()
    val time = createdAtSecs * MILLIS_PER_SECOND
    return if (forumNotificationAgeIsRelative(createdAtSecs, now / MILLIS_PER_SECOND)) {
        DateUtils.getRelativeTimeSpanString(
                // A clock skew between the forum host and this device must not
                // produce "in 3 seconds" on a notification that already exists.
                minOf(time, now),
                now,
                DateUtils.MINUTE_IN_MILLIS,
                DateUtils.FORMAT_ABBREV_RELATIVE,
            )
            .toString()
    } else {
        DateUtils.formatDateTime(
            context,
            time,
            DateUtils.FORMAT_SHOW_DATE or DateUtils.FORMAT_SHOW_YEAR or DateUtils.FORMAT_ABBREV_MONTH,
        )
    }
}
