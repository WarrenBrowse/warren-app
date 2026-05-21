package com.warrenbrowse.vpn.feature.appearance.impl

import android.os.Build
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.common.compose.itemWithDivider
import com.warrenbrowse.vpn.common.compose.unlessIsDetail
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.appicon.api.AppIconNavKey
import com.warrenbrowse.vpn.feature.language.api.LanguageNavKey
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.component.listitem.NavigationListItem
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens

@OptIn(ExperimentalMaterial3Api::class)
@Preview
@Composable
private fun PreviewAppearanceScreen() {
    AppTheme { AppearanceScreen(onAppIconClick = {}, onLanguageClick = {}, onBackClick = {}) }
}

@Composable
fun Appearance(navigator: Navigator) {
    AppearanceScreen(
        onAppIconClick = dropUnlessResumed { navigator.navigate(AppIconNavKey) },
        onLanguageClick =
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                dropUnlessResumed { navigator.navigate(LanguageNavKey) }
            } else {
                null
            },
        onBackClick = dropUnlessResumed { navigator.goBack() },
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AppearanceScreen(
    onAppIconClick: () -> Unit,
    onLanguageClick: (() -> Unit)?,
    onBackClick: () -> Unit,
) {
    ScaffoldWithSmallTopBar(
        appBarTitle = stringResource(id = R.string.appearance),
        navigationIcon = { unlessIsDetail { NavigateBackIconButton(onNavigateBack = onBackClick) } },
    ) { modifier ->
        val lazyListState: LazyListState = rememberLazyListState()
        LazyColumn(
            modifier = modifier.padding(horizontal = Dimens.sideMarginNew),
            state = lazyListState,
        ) {
            itemWithDivider {
                NavigationListItem(
                    title = stringResource(id = R.string.app_icon),
                    onClick = onAppIconClick,
                    position = if (onLanguageClick != null) Position.Top else Position.Single,
                )
            }
            if (onLanguageClick != null) {
                item {
                    NavigationListItem(
                        title = stringResource(id = R.string.language),
                        onClick = onLanguageClick,
                        position = Position.Bottom,
                    )
                }
            }
        }
    }
}
