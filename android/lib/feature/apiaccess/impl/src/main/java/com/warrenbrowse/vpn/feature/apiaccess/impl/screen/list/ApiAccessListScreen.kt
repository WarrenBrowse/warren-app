package com.warrenbrowse.vpn.feature.apiaccess.impl.screen.list

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Info
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.tooling.preview.PreviewParameter
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.warrenbrowse.vpn.common.compose.itemsIndexedWithDivider
import com.warrenbrowse.vpn.common.compose.unlessIsDetail
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.apiaccess.api.ApiAccessMethodDetailsNavKey
import com.warrenbrowse.vpn.feature.apiaccess.api.ApiAccessMethodInfoNavKey
import com.warrenbrowse.vpn.feature.apiaccess.api.EditApiAccessMethodNavKey
import com.warrenbrowse.vpn.feature.apiaccess.impl.util.toDisplayName
import com.warrenbrowse.vpn.lib.model.ApiAccessMethodSetting
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.component.drawVerticalScrollbar
import com.warrenbrowse.vpn.lib.ui.component.listitem.NavigationListItem
import com.warrenbrowse.vpn.lib.ui.component.positionForIndex
import com.warrenbrowse.vpn.lib.ui.component.text.ScreenDescription
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.tag.API_ACCESS_LIST_INFO_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.AlphaScrollbar
import org.koin.androidx.compose.koinViewModel

@Preview("Default|WithoutCustomApi|WithCustomApi")
@Composable
private fun PreviewApiAccessList(
    @PreviewParameter(ApiAccessListUiStatePreviewParameterProvider::class)
    state: ApiAccessListUiState
) {
    AppTheme {
        ApiAccessListScreen(
            state = state,
            onAddMethodClick = {},
            onApiAccessMethodClick = { _ -> },
            onApiAccessInfoClick = {},
            onBackClick = {},
        )
    }
}

@Composable
fun ApiAccessList(navigator: Navigator) {
    val viewModel = koinViewModel<ApiAccessListViewModel>()
    val state by viewModel.uiState.collectAsStateWithLifecycle()

    ApiAccessListScreen(
        state = state,
        onAddMethodClick = { navigator.navigate(EditApiAccessMethodNavKey()) },
        onApiAccessMethodClick = { navigator.navigate(ApiAccessMethodDetailsNavKey(it.id)) },
        onApiAccessInfoClick = { navigator.navigate(ApiAccessMethodInfoNavKey) },
        onBackClick = navigator::goBack,
    )
}

@Composable
fun ApiAccessListScreen(
    state: ApiAccessListUiState,
    onAddMethodClick: () -> Unit,
    onApiAccessMethodClick: (apiAccessMethodSetting: ApiAccessMethodSetting) -> Unit,
    onApiAccessInfoClick: () -> Unit,
    onBackClick: () -> Unit,
) {
    ScaffoldWithSmallTopBar(
        appBarTitle = stringResource(id = R.string.settings_api_access),
        navigationIcon = { unlessIsDetail { NavigateBackIconButton(onNavigateBack = onBackClick) } },
    ) { modifier ->
        val lazyListState: LazyListState = rememberLazyListState()
        LazyColumn(
            modifier =
                modifier
                    .drawVerticalScrollbar(
                        state = lazyListState,
                        color = MaterialTheme.colorScheme.onSurface.copy(alpha = AlphaScrollbar),
                    )
                    .padding(horizontal = Dimens.sideMarginNew),
            state = lazyListState,
        ) {
            description()
            currentAccessMethod(
                currentApiAccessMethodSetting = state.currentApiAccessMethodSetting,
                onInfoClicked = onApiAccessInfoClick,
            )
            apiAccessMethodItems(
                state.apiAccessMethodSettings,
                onApiAccessMethodClick = onApiAccessMethodClick,
            )
            buttonPanel(onAddMethodClick = onAddMethodClick)
        }
    }
}

private fun LazyListScope.description() {
    item {
        ScreenDescription(
            text = stringResource(id = R.string.api_access_description),
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

private fun LazyListScope.currentAccessMethod(
    currentApiAccessMethodSetting: ApiAccessMethodSetting?,
    onInfoClicked: () -> Unit,
) {
    item {
        Row(
            modifier = Modifier.padding(top = Dimens.tinyPadding, bottom = Dimens.largeSpacer),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                style = MaterialTheme.typography.titleMedium,
                color = MaterialTheme.colorScheme.onSurface,
                text =
                    stringResource(
                        id = R.string.current_method,
                        currentApiAccessMethodSetting.toDisplayName(),
                    ),
            )
            IconButton(
                onClick = onInfoClicked,
                modifier =
                    Modifier.align(Alignment.CenterVertically)
                        .testTag(API_ACCESS_LIST_INFO_TEST_TAG),
            ) {
                Icon(
                    imageVector = Icons.Rounded.Info,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurface,
                )
            }
        }
    }
}

private fun LazyListScope.apiAccessMethodItems(
    apiAccessMethodSettings: List<ApiAccessMethodSetting>,
    onApiAccessMethodClick: (apiAccessMethodSetting: ApiAccessMethodSetting) -> Unit,
) {
    itemsIndexedWithDivider(
        items = apiAccessMethodSettings,
        key = { _, item -> item.id },
        contentType = { _, _ -> ContentType.ITEM },
    ) { index, item ->
        ApiAccessMethodItem(
            apiAccessMethodSetting = item,
            position = apiAccessMethodSettings.positionForIndex(index),
            onApiAccessMethodClick = onApiAccessMethodClick,
        )
    }
}

@Composable
private fun ApiAccessMethodItem(
    apiAccessMethodSetting: ApiAccessMethodSetting,
    position: Position,
    onApiAccessMethodClick: (apiAccessMethodSetting: ApiAccessMethodSetting) -> Unit,
) {
    NavigationListItem(
        title = apiAccessMethodSetting.toDisplayName(),
        subtitle =
            stringResource(
                id =
                    if (apiAccessMethodSetting.enabled) {
                        R.string.on
                    } else {
                        R.string.off
                    }
            ),
        onClick = { onApiAccessMethodClick(apiAccessMethodSetting) },
        position = position,
    )
}

private fun LazyListScope.buttonPanel(onAddMethodClick: () -> Unit) {
    item {
        PrimaryButton(
            modifier =
                Modifier.padding(horizontal = Dimens.smallPadding, vertical = Dimens.largePadding),
            onClick = onAddMethodClick,
            text = stringResource(id = R.string.add),
        )
    }
}

object ContentType {
    const val ITEM = 2
}
