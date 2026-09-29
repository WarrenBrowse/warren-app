package com.warrenbrowse.vpn.feature.settings.impl

import androidx.annotation.StringRes
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import com.warrenbrowse.vpn.common.compose.itemWithDivider
import com.warrenbrowse.vpn.lib.model.ThemePreference
import com.warrenbrowse.vpn.lib.ui.component.listitem.ExpandableListItem
import com.warrenbrowse.vpn.lib.ui.component.listitem.SelectableListItem
import com.warrenbrowse.vpn.lib.ui.designsystem.Hierarchy
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens

/** The choices, in the desktop order: following the device first, it is right for almost everyone. */
val themeOptions: List<ThemePreference> =
    listOf(ThemePreference.SYSTEM, ThemePreference.DARK, ThemePreference.LIGHT)

@StringRes
fun ThemePreference.labelRes(): Int =
    when (this) {
        ThemePreference.SYSTEM -> R.string.theme_system
        ThemePreference.DARK -> R.string.theme_dark
        ThemePreference.LIGHT -> R.string.theme_light
    }

/**
 * The theme row of the user interface group (desktop ThemeSetting): folded, with the current
 * choice in the header, since the three options only earn their space when overridden.
 */
internal fun LazyListScope.themeSetting(
    preference: ThemePreference,
    expanded: Boolean,
    onToggle: (Boolean) -> Unit,
    onSelect: (ThemePreference) -> Unit,
) {
    itemWithDivider(key = "theme_setting") {
        ExpandableListItem(
            position = Position.Top,
            isExpanded = expanded,
            onCellClicked = onToggle,
            content = {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(text = stringResource(R.string.theme), modifier = Modifier.weight(1f))
                    Text(
                        text = stringResource(preference.labelRes()),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(start = Dimens.smallPadding),
                    )
                }
            },
        )
    }
    if (expanded) {
        themeOptions.forEach { option ->
            itemWithDivider(key = "theme_option_${option.storageValue}") {
                SelectableListItem(
                    hierarchy = Hierarchy.Child1,
                    position = Position.Middle,
                    isSelected = option == preference,
                    title = stringResource(option.labelRes()),
                    onClick = { onSelect(option) },
                )
            }
        }
    }
}
