package com.warrenbrowse.vpn.feature.splittunneling.impl

import android.content.pm.PackageManager
import android.graphics.drawable.Drawable
import androidx.compose.animation.AnimatedVisibilityScope
import androidx.compose.animation.SharedTransitionScope
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyItemScope
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.retain.retain
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.ui.focus.FocusManager
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.tooling.preview.PreviewParameter
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.compose.dropUnlessResumed
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import com.warrenbrowse.vpn.common.compose.unlessIsDetail
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.splittunneling.api.SearchSplitTunnelingNavKey
import com.warrenbrowse.vpn.feature.splittunneling.impl.applist.AppData
import com.warrenbrowse.vpn.feature.splittunneling.impl.extensions.hasValidSize
import com.warrenbrowse.vpn.feature.splittunneling.impl.extensions.isBelowMaxByteSize
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.model.FeatureIndicator
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.model.SplitTunnelMode
import com.warrenbrowse.vpn.lib.ui.component.dialog.InfoConfirmationDialog
import com.warrenbrowse.vpn.lib.ui.component.dialog.InfoConfirmationDialogTitleType
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateCloseIconButton
import com.warrenbrowse.vpn.lib.ui.component.button.SearchButton
import com.warrenbrowse.vpn.lib.ui.component.drawVerticalScrollbar
import com.warrenbrowse.vpn.lib.ui.component.listitem.IconState
import com.warrenbrowse.vpn.lib.ui.component.listitem.SplitTunnelingListItem
import com.warrenbrowse.vpn.lib.ui.component.listitem.SwitchListItem
import com.warrenbrowse.vpn.lib.ui.component.text.ScreenDescription
import com.warrenbrowse.vpn.lib.ui.designsystem.ListHeader
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenCircularProgressIndicatorLarge
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.AlphaDisabled
import com.warrenbrowse.vpn.lib.ui.theme.color.AlphaScrollbar
import com.warrenbrowse.vpn.lib.ui.theme.color.AlphaVisible
import com.warrenbrowse.vpn.lib.ui.theme.color.warning
import com.warrenbrowse.vpn.lib.ui.util.visible
import org.koin.androidx.compose.koinViewModel
import org.koin.core.parameter.parametersOf

@Preview("ShowAppList|Loading")
@Composable
private fun PreviewSplitTunnelingScreen(
    @PreviewParameter(SplitTunnelingUiStatePreviewParameterProvider::class)
    state: Lc<Loading, SplitTunnelingUiState>
) {
    AppTheme {
        SplitTunnelingScreen(
            state = state,
            onSelectTab = {},
            onSplitModeSwitch = {},
            onConfirmModeChange = {},
            onCancelModeChange = {},
            onShowSystemAppsClick = {},
            onAddAppClick = {},
            onRemoveAppClick = {},
            onBackClick = {},
            navigateToSearch = {},
            onResolveIcon = { null },
        )
    }
}

@Composable
fun SharedTransitionScope.SplitTunneling(
    isModal: Boolean,
    navigator: Navigator,
    animatedVisibilityScope: AnimatedVisibilityScope,
) {
    val viewModel = koinViewModel<SplitTunnelingViewModel> { parametersOf(isModal) }
    val state by viewModel.uiState.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val packageManager = remember(context) { context.packageManager }

    SplitTunnelingScreen(
        state = state,
        modifier =
            Modifier.sharedBounds(
                rememberSharedContentState(key = FeatureIndicator.SPLIT_TUNNELING),
                animatedVisibilityScope = animatedVisibilityScope,
            ),
        onSelectTab = viewModel::onSelectTab,
        onSplitModeSwitch = viewModel::onSplitModeSwitch,
        onConfirmModeChange = viewModel::onConfirmModeChange,
        onCancelModeChange = viewModel::onCancelModeChange,
        onShowSystemAppsClick = viewModel::onShowSystemAppsClick,
        onAddAppClick = viewModel::onAddAppClick,
        onRemoveAppClick = viewModel::onRemoveAppClick,
        onBackClick = dropUnlessResumed { navigator.goBack() },
        navigateToSearch =
            dropUnlessResumed {
                val includeOnly =
                    (state as? Lc.Content)?.value?.tab == SplitTunnelingTab.IncludeOnly
                navigator.navigate(SearchSplitTunnelingNavKey(includeOnly = includeOnly))
            },
        onResolveIcon = { packageName -> packageManager.getApplicationIconOrNull(packageName) },
    )
}

@Composable
@Suppress("LongParameterList")
fun SplitTunnelingScreen(
    state: Lc<Loading, SplitTunnelingUiState>,
    onSelectTab: (SplitTunnelingTab) -> Unit,
    onSplitModeSwitch: (Boolean) -> Unit,
    onConfirmModeChange: () -> Unit,
    onCancelModeChange: () -> Unit,
    onShowSystemAppsClick: (show: Boolean) -> Unit,
    onAddAppClick: (packageName: PackageName) -> Unit,
    onRemoveAppClick: (packageName: PackageName) -> Unit,
    onBackClick: () -> Unit,
    onResolveIcon: (PackageName) -> Drawable?,
    navigateToSearch: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val focusManager = LocalFocusManager.current

    ScaffoldWithSmallTopBar(
        modifier = modifier.fillMaxSize(),
        appBarTitle = stringResource(id = R.string.split_tunneling),
        navigationIcon = {
            if (state.isModal()) {
                NavigateCloseIconButton(onNavigateClose = onBackClick)
            } else {
                unlessIsDetail { NavigateBackIconButton(onNavigateBack = onBackClick) }
            }
        },
        actions = { SearchButton(onClick = navigateToSearch, enabled = state is Lc.Content) },
    ) { modifier ->
        val lazyListState = rememberLazyListState()
        LazyColumn(
            modifier =
                modifier
                    .drawVerticalScrollbar(
                        state = lazyListState,
                        color = MaterialTheme.colorScheme.onSurface.copy(alpha = AlphaScrollbar),
                    )
                    .background(MaterialTheme.colorScheme.surface)
                    .padding(horizontal = Dimens.sideMarginNew),
            horizontalAlignment = Alignment.CenterHorizontally,
            state = lazyListState,
        ) {
            description()
            when (state) {
                is Lc.Loading -> {
                    spacer()
                    loading()
                }
                is Lc.Content -> {
                    tabBar(tab = state.value.tab, onSelectTab = onSelectTab)
                    modeSwitch(state = state.value, onSplitModeSwitch = onSplitModeSwitch)
                    tabDescription(tab = state.value.tab)
                    if (state.value.tab == SplitTunnelingTab.IncludeOnly && state.value.tabModeOn) {
                        includeOnlyBanner(noApp = state.value.includeOnlyWithoutApps)
                    }
                    systemAppsToggle(
                        showSystemApps = state.value.showSystemApps,
                        onShowSystemAppsClick = onShowSystemAppsClick,
                        enabled = true,
                    )
                    appList(
                        state = state.value,
                        focusManager = focusManager,
                        onAddAppClick = onAddAppClick,
                        onRemoveAppClick = onRemoveAppClick,
                        onResolveIcon = onResolveIcon,
                    )
                }
            }
        }
    }

    (state as? Lc.Content)?.value?.confirmation?.let { confirmation ->
        ModeChangeDialog(
            confirmation = confirmation,
            onConfirm = onConfirmModeChange,
            onCancel = onCancelModeChange,
        )
    }
}

private fun LazyListScope.description() {
    item(key = CommonContentKey.DESCRIPTION, contentType = ContentType.DESCRIPTION) {
        ScreenDescription(
            text = stringResource(id = R.string.app_routing_description),
            modifier = Modifier.padding(bottom = Dimens.mediumPadding),
        )
    }
}

/** "Bypass VPN" and "VPN only for", in the desktop's order. */
private fun LazyListScope.tabBar(
    tab: SplitTunnelingTab,
    onSelectTab: (SplitTunnelingTab) -> Unit,
) {
    item(key = SplitTunnelingContentKey.TABS, contentType = ContentType.OTHER_ITEM) {
        SingleChoiceSegmentedButtonRow(
            modifier = Modifier.fillMaxWidth().padding(bottom = Dimens.mediumPadding)
        ) {
            SplitTunnelingTab.entries.forEachIndexed { index, entry ->
                SegmentedButton(
                    selected = tab == entry,
                    onClick = { onSelectTab(entry) },
                    shape =
                        SegmentedButtonDefaults.itemShape(
                            index = index,
                            count = SplitTunnelingTab.entries.size,
                        ),
                ) {
                    Text(stringResource(entry.label()))
                }
            }
        }
    }
}

private fun LazyListScope.modeSwitch(
    state: SplitTunnelingUiState,
    onSplitModeSwitch: (Boolean) -> Unit,
) {
    item(key = SplitTunnelingContentKey.MODE_SWITCH, contentType = ContentType.OTHER_ITEM) {
        SwitchListItem(
            title = stringResource(id = state.tab.label()),
            isToggled = state.tabModeOn,
            onCellClicked = onSplitModeSwitch,
            position = Position.Single,
            modifier = Modifier.animateItem(),
        )
    }
}

private fun LazyListScope.tabDescription(tab: SplitTunnelingTab) {
    item(key = SplitTunnelingContentKey.TAB_DESCRIPTION, contentType = ContentType.DESCRIPTION) {
        ScreenDescription(
            text =
                when (tab) {
                    SplitTunnelingTab.Bypass ->
                        stringResource(R.string.split_mode_bypass_description) +
                            "\n" +
                            stringResource(R.string.split_tunneling_description_warning)
                    SplitTunnelingTab.IncludeOnly ->
                        stringResource(R.string.split_mode_include_only_description) +
                            "\n" +
                            stringResource(R.string.include_only_lockdown_warning)
                },
            modifier =
                Modifier.animateItem()
                    .padding(top = Dimens.smallPadding, bottom = Dimens.mediumPadding),
        )
    }
}

/**
 * Stays for as long as include-only is on: the rest of the device is not
 * protected, and that should never be a surprise. With no chosen app on the
 * device it says instead that every app uses the VPN.
 */
private fun LazyListScope.includeOnlyBanner(noApp: Boolean) {
    item(key = SplitTunnelingContentKey.INCLUDE_ONLY_BANNER, contentType = ContentType.OTHER_ITEM) {
        val shape = RoundedCornerShape(Dimens.smallPadding)
        Row(
            modifier =
                Modifier.animateItem()
                    .fillMaxWidth()
                    .padding(bottom = Dimens.mediumPadding)
                    .background(MaterialTheme.colorScheme.warning.copy(alpha = BANNER_FILL), shape)
                    .border(
                        Dimens.thinBorderWidth,
                        MaterialTheme.colorScheme.warning.copy(alpha = BANNER_BORDER),
                        shape,
                    )
                    .padding(Dimens.smallPadding)
                    .semantics(mergeDescendants = true) {},
            horizontalArrangement = Arrangement.spacedBy(Dimens.smallPadding),
        ) {
            Icon(
                painter = painterResource(R.drawable.ic_forum_alert_circle),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.warning,
                modifier = Modifier.size(Dimens.smallIconSize),
            )
            Text(
                text =
                    stringResource(
                        if (noApp) R.string.include_only_no_app else R.string.include_only_banner
                    ),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurface,
            )
        }
    }
}

@Composable
private fun ModeChangeDialog(
    confirmation: ModeChangeConfirmation,
    onConfirm: () -> Unit,
    onCancel: () -> Unit,
) {
    InfoConfirmationDialog(
        onResult = { confirmed -> if (confirmed != null) onConfirm() else onCancel() },
        titleType = InfoConfirmationDialogTitleType.IconOnly,
        confirmButtonTitle = stringResource(R.string.split_mode_turn_on),
        cancelButtonTitle = stringResource(R.string.cancel),
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(Dimens.verticalSpace)) {
            if (confirmation.leavesDeviceUnprotected) {
                DialogText(stringResource(R.string.include_only_confirm))
            }
            when (confirmation.replaces) {
                SplitTunnelMode.Exclude ->
                    DialogText(stringResource(R.string.split_mode_replaces_bypass))
                SplitTunnelMode.IncludeOnly ->
                    DialogText(stringResource(R.string.split_mode_replaces_include_only))
                SplitTunnelMode.Off,
                null -> Unit
            }
        }
    }
}

@Composable
private fun DialogText(text: String) {
    Text(
        text = text,
        color = MaterialTheme.colorScheme.onSurface,
        style = MaterialTheme.typography.bodySmall,
        modifier = Modifier.fillMaxWidth(),
    )
}

private fun SplitTunnelingTab.label(): Int =
    when (this) {
        SplitTunnelingTab.Bypass -> R.string.split_mode_bypass
        SplitTunnelingTab.IncludeOnly -> R.string.split_mode_include_only
    }

/** The header over the apps on the list of this tab. */
internal fun SplitTunnelingTab.selectedAppsHeader(): Int =
    when (this) {
        SplitTunnelingTab.Bypass -> R.string.exclude_applications
        SplitTunnelingTab.IncludeOnly -> R.string.apps_using_the_vpn
    }

private const val BANNER_FILL = 0.12f
private const val BANNER_BORDER = 0.45f

private fun LazyListScope.loading() {
    item(key = CommonContentKey.PROGRESS, contentType = ContentType.PROGRESS) {
        WarrenCircularProgressIndicatorLarge()
    }
}

private fun LazyListScope.appList(
    state: SplitTunnelingUiState,
    focusManager: FocusManager,
    onAddAppClick: (packageName: PackageName) -> Unit,
    onRemoveAppClick: (packageName: PackageName) -> Unit,
    onResolveIcon: (PackageName) -> Drawable?,
) {
    // Both lists stay editable while their mode is off, so the apps can be
    // chosen before "VPN only for" is turned on rather than after.
    if (state.selectedApps.isNotEmpty()) {
        selectedAppsHeaderItem(
            key = SplitTunnelingContentKey.SELECTED_APPLICATIONS,
            textId = state.tab.selectedAppsHeader(),
            enabled = true,
            selectedAppsCount = state.selectedApps.size,
            otherAppsCount = state.otherApps.size,
        )
        appItems(
            apps = state.selectedApps,
            focusManager = focusManager,
            onAppClick = onRemoveAppClick,
            onResolveIcon = onResolveIcon,
            enabled = true,
            selected = true,
        )
    }
    spacer()
    headerItem(
        key = SplitTunnelingContentKey.OTHER_APPLICATIONS,
        textId = R.string.all_applications,
        enabled = true,
    )
    appItems(
        apps = state.otherApps,
        focusManager = focusManager,
        onAppClick = onAddAppClick,
        onResolveIcon = onResolveIcon,
        enabled = true,
        selected = false,
    )
    spacer()
}

internal fun LazyListScope.appItems(
    apps: List<AppData>,
    focusManager: FocusManager,
    onAppClick: (PackageName) -> Unit,
    onResolveIcon: (PackageName) -> Drawable?,
    enabled: Boolean,
    selected: Boolean,
) {
    itemsIndexedWithDivider(
        items = apps,
        key = { _, listItem -> listItem.packageName.value },
        contentType = { _, _ -> ContentType.ITEM },
    ) { index, listItem ->
        val packageName = listItem.packageName
        var icon by retain(packageName) { mutableStateOf<IconState>(IconState.Loading) }
        LaunchedEffect(packageName) {
            launch(Dispatchers.IO) {
                val drawable = onResolveIcon(packageName)
                icon =
                    if (
                        drawable != null && drawable.isBelowMaxByteSize() && drawable.hasValidSize()
                    ) {
                        IconState.Icon(drawable = drawable)
                    } else {
                        IconState.NoIcon
                    }
            }
        }
        SplitTunnelingListItem(
            title = listItem.name,
            iconState = icon,
            isSelected = selected,
            isEnabled = enabled,
            modifier = Modifier.animateItem(),
            position =
                when (index) {
                    0 if apps.size == 1 -> Position.Single
                    0 -> Position.Top
                    apps.lastIndex -> Position.Bottom
                    else -> Position.Middle
                },
            backgroundAlpha =
                if (enabled) {
                    AlphaVisible
                } else {
                    AlphaDisabled
                },
        ) {
            // Move focus down unless the clicked item was the last in this
            // section.
            if (index < apps.size - 1) {
                focusManager.moveFocus(FocusDirection.Down)
            } else {
                focusManager.moveFocus(FocusDirection.Up)
            }

            onAppClick(listItem.packageName)
        }
    }
}

internal fun LazyListScope.headerItem(key: String, textId: Int, enabled: Boolean) {
    itemWithDivider(key = key, contentType = ContentType.HEADER) {
        ListHeader(
            modifier = Modifier.animateItem().visible(enabled),
            text = stringResource(id = textId),
        )
    }
}

internal fun LazyListScope.selectedAppsHeaderItem(
    key: String,
    textId: Int,
    enabled: Boolean,
    selectedAppsCount: Int,
    otherAppsCount: Int,
) {
    itemWithDivider(key = key, contentType = ContentType.HEADER) {
        ListHeader(
            modifier = Modifier.animateItem().visible(enabled),
            text = stringResource(id = textId),
            trailingText =
                stringResource(
                    R.string.x_out_of_y,
                    selectedAppsCount,
                    selectedAppsCount + otherAppsCount,
                ),
        )
    }
}

internal fun LazyListScope.systemAppsToggle(
    showSystemApps: Boolean,
    onShowSystemAppsClick: (show: Boolean) -> Unit,
    enabled: Boolean,
) {
    itemWithDivider(
        key = SplitTunnelingContentKey.SHOW_SYSTEM_APPLICATIONS,
        contentType = ContentType.OTHER_ITEM,
    ) {
        SwitchListItem(
            title = stringResource(id = R.string.show_system_apps),
            isToggled = showSystemApps,
            onCellClicked = { newValue -> onShowSystemAppsClick(newValue) },
            isEnabled = enabled,
            modifier = Modifier.animateItem(),
            backgroundAlpha =
                if (enabled) {
                    AlphaVisible
                } else {
                    AlphaDisabled
                },
            position = Position.Single,
        )
    }
}

private fun LazyListScope.spacer() {
    item(contentType = ContentType.SPACER) {
        Spacer(modifier = Modifier.animateItem().height(Dimens.cellVerticalSpacing))
    }
}

private fun Lc<Loading, SplitTunnelingUiState>.isModal(): Boolean =
    when (this) {
        is Lc.Loading -> value.isModal
        is Lc.Content -> value.isModal
    }

fun PackageManager.getApplicationIconOrNull(packageName: PackageName): Drawable? =
    try {
        getApplicationIcon(packageName.value)
    } catch (e: PackageManager.NameNotFoundException) {
        // Name not found is thrown if the application is not installed
        null
    } catch (e: IllegalArgumentException) {
        // IllegalArgumentException is thrown if the application has an invalid icon
        null
    } catch (e: OutOfMemoryError) {
        // OutOfMemoryError is thrown if the icon is too large
        null
    }

object CommonContentKey {
    const val DESCRIPTION = "description"
    const val PROGRESS = "progress"
}

private inline fun <T> LazyListScope.itemsIndexedWithDivider(
    items: List<T>,
    noinline key: ((index: Int, item: T) -> Any)? = null,
    crossinline contentType: (index: Int, item: T) -> Any? = { _, _ -> null },
    crossinline itemContent: @Composable LazyItemScope.(index: Int, item: T) -> Unit,
) =
    itemsIndexed(items = items, key = key, contentType = contentType) { index, item ->
        itemContent(index, item)
        HorizontalDivider(color = Color.Transparent)
    }

private inline fun LazyListScope.itemWithDivider(
    key: Any? = null,
    contentType: Any? = null,
    crossinline itemContent: @Composable LazyItemScope.() -> Unit,
) =
    item(key = key, contentType = contentType) {
        itemContent()
        HorizontalDivider(color = Color.Transparent)
    }

internal object SplitTunnelingContentKey {
    const val TABS = "tabs"
    const val MODE_SWITCH = "mode_switch"
    const val TAB_DESCRIPTION = "tab_description"
    const val INCLUDE_ONLY_BANNER = "include_only_banner"
    const val SELECTED_APPLICATIONS = "selected"
    const val SHOW_SYSTEM_APPLICATIONS = "show_system"
    const val OTHER_APPLICATIONS = "others"
}
