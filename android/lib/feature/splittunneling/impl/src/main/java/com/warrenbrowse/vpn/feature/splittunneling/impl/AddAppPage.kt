package com.warrenbrowse.vpn.feature.splittunneling.impl

import android.graphics.drawable.Drawable
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material3.MaterialTheme
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenSwitch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.ui.component.drawVerticalScrollbar
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.AlphaScrollbar

/**
 * The apps without a rule. Picking one opens its route, and no rule exists until a route other
 * than the default is chosen there.
 */
@Composable
internal fun AddAppPage(
    page: AppRoutingPage.AddApp,
    actions: AppRoutingActions,
    onResolveIcon: (PackageName) -> Drawable?,
) {
    SubPage(
        title = stringResource(R.string.app_routing_add_app),
        scrolls = false,
        buttons = {
            PrimaryButton(
                onClick = actions.onBack,
                text = stringResource(R.string.cancel),
                modifier = Modifier.fillMaxWidth(),
            )
        },
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(Dimens.smallPadding)) {
            SearchField(query = page.searchTerm, onQueryChange = actions.onAddAppSearchChange)
            SystemAppsToggle(page.showSystemApps, actions.onShowSystemApps)
            if (page.apps.isEmpty() && page.searchTerm.isNotBlank()) {
                Text(
                    text =
                        stringResource(R.string.search_no_matches_for_text, page.searchTerm.trim()),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(vertical = Dimens.mediumPadding),
                )
            }
            val listState = rememberLazyListState()
            LazyColumn(
                state = listState,
                modifier =
                    Modifier.fillMaxSize()
                        .drawVerticalScrollbar(
                            state = listState,
                            color = MaterialTheme.colorScheme.onSurface.copy(alpha = AlphaScrollbar),
                        ),
                verticalArrangement = Arrangement.spacedBy(Dimens.smallPadding),
            ) {
                items(items = page.apps, key = { it.packageName.value }) { app ->
                    Row(
                        modifier =
                            Modifier.fillMaxWidth()
                                .heightIn(min = 56.dp)
                                .clip(CardShape)
                                .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                                .clickable { actions.onOpenApp(app) }
                                .semantics(mergeDescendants = true) { role = Role.Button }
                                .padding(horizontal = Dimens.cellStartPadding - 2.dp, vertical = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(Dimens.cellStartPadding - 4.dp),
                    ) {
                        AppIcon(app.packageName, onResolveIcon, AppIconSize)
                        Text(
                            text = app.name,
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.onSurface,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun SystemAppsToggle(show: Boolean, onToggle: (Boolean) -> Unit) {
    Row(
        modifier =
            Modifier.fillMaxWidth()
                .toggleable(value = show, role = Role.Switch, onValueChange = onToggle)
                .padding(vertical = Dimens.tinyPadding),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = stringResource(R.string.show_system_apps),
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.weight(1f),
        )
        WarrenSwitch(checked = show, onCheckedChange = null)
    }
}
