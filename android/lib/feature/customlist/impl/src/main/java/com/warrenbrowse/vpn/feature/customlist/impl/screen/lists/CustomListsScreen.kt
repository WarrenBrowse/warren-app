package com.warrenbrowse.vpn.feature.customlist.impl.screen.lists

import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.tooling.preview.PreviewParameter
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.compose.dropUnlessResumed
import kotlinx.coroutines.launch
import com.warrenbrowse.vpn.common.compose.dropUnlessResumed
import com.warrenbrowse.vpn.common.compose.itemsIndexedWithDivider
import com.warrenbrowse.vpn.common.compose.showSnackbarImmediately
import com.warrenbrowse.vpn.core.LocalResultStore
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.customlist.api.CreateCustomListNavKey
import com.warrenbrowse.vpn.feature.customlist.api.EditCustomListNavKey
import com.warrenbrowse.vpn.feature.customlist.api.EditCustomListNavResult
import com.warrenbrowse.vpn.lib.model.CustomList
import com.warrenbrowse.vpn.lib.model.communication.CustomListActionResultData
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.component.drawVerticalScrollbar
import com.warrenbrowse.vpn.lib.ui.component.listitem.NavigationListItem
import com.warrenbrowse.vpn.lib.ui.component.positionForIndex
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenCircularProgressIndicatorLarge
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.tag.NEW_LIST_BUTTON_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.AlphaScrollbar
import org.koin.androidx.compose.koinViewModel

@Preview("Content|Empty|Loading")
@Composable
private fun PreviewAccountScreen(
    @PreviewParameter(CustomListsUiStatePreviewParameterProvider::class) state: CustomListsUiState
) {
    AppTheme {
        CustomListsScreen(
            state = state,
            snackbarHostState = SnackbarHostState(),
            addCustomList = {},
            openCustomList = { _ -> },
            onBackClick = {},
        )
    }
}

@Composable
fun CustomLists(navigator: Navigator) {
    val viewModel = koinViewModel<CustomListsViewModel>()
    val state by viewModel.uiState.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    val resources = LocalResources.current
    val snackbarHostState = remember { SnackbarHostState() }

    LocalResultStore.current.consumeResult<EditCustomListNavResult> { result ->
        val value = result.value
        if (value is CustomListActionResultData.Success.Deleted) {
            scope.launch {
                snackbarHostState.showSnackbarImmediately(
                    message =
                        resources.getString(
                            R.string.delete_custom_list_message,
                            value.customListName,
                        ),
                    actionLabel = resources.getString(R.string.undo),
                    duration = SnackbarDuration.Long,
                    onAction = { viewModel.undoDeleteCustomList(value.undo) },
                )
            }
        }
    }

    CustomListsScreen(
        state = state,
        snackbarHostState = snackbarHostState,
        addCustomList = dropUnlessResumed { navigator.navigate(CreateCustomListNavKey(null)) },
        openCustomList =
            dropUnlessResumed { customList ->
                navigator.navigate(EditCustomListNavKey(customListId = customList.id))
            },
        onBackClick = dropUnlessResumed { navigator.goBack() },
    )
}

@Composable
fun CustomListsScreen(
    state: CustomListsUiState,
    snackbarHostState: SnackbarHostState = remember { SnackbarHostState() },
    addCustomList: () -> Unit,
    openCustomList: (CustomList) -> Unit,
    onBackClick: () -> Unit,
) {
    ScaffoldWithSmallTopBar(
        appBarTitle = stringResource(id = R.string.edit_custom_lists),
        navigationIcon = { NavigateBackIconButton(onNavigateBack = onBackClick) },
        actions = {
            IconButton(
                onClick = addCustomList,
                modifier = Modifier.testTag(NEW_LIST_BUTTON_TEST_TAG),
            ) {
                Icon(
                    imageVector = Icons.Rounded.Add,
                    tint = MaterialTheme.colorScheme.onSurface,
                    contentDescription = stringResource(id = R.string.new_list),
                )
            }
        },
        snackbarHostState = snackbarHostState,
    ) { modifier: Modifier ->
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
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            when (state) {
                is CustomListsUiState.Content -> {
                    if (state.customLists.isNotEmpty()) {
                        content(customLists = state.customLists, openCustomList = openCustomList)
                    } else {
                        empty()
                    }
                }
                is CustomListsUiState.Loading -> {
                    loading()
                }
            }
        }
    }
}

private fun LazyListScope.loading() {
    item(contentType = ContentType.PROGRESS) { WarrenCircularProgressIndicatorLarge() }
}

private fun LazyListScope.content(
    customLists: List<CustomList>,
    openCustomList: (CustomList) -> Unit,
) {
    itemsIndexedWithDivider(
        items = customLists,
        key = { _, item: CustomList -> item.id },
        contentType = { _, _ -> ContentType.ITEM },
    ) { index, customList ->
        NavigationListItem(
            title = customList.name.value,
            position = customLists.positionForIndex(index),
            onClick = { openCustomList(customList) },
        )
    }
}

private fun LazyListScope.empty() {
    item(contentType = ContentType.EMPTY_TEXT) {
        Text(
            text = stringResource(R.string.no_custom_lists_available),
            modifier = Modifier.padding(Dimens.mediumPadding),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

object ContentType {
    const val ITEM = 2
    const val PROGRESS = 6
    const val EMPTY_TEXT = 7
}
