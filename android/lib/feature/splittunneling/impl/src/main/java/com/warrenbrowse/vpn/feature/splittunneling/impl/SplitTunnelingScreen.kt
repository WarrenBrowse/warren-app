package com.warrenbrowse.vpn.feature.splittunneling.impl

import android.content.pm.PackageManager
import android.graphics.drawable.Drawable
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibilityScope
import androidx.compose.animation.SharedTransitionScope
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.WarningAmber
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.common.compose.unlessIsDetail
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.splittunneling.impl.countries.CountryPickerActions
import com.warrenbrowse.vpn.feature.splittunneling.impl.countries.CountryPickerPage
import com.warrenbrowse.vpn.lib.common.Lc
import com.warrenbrowse.vpn.lib.model.DefaultRoute
import com.warrenbrowse.vpn.lib.model.FeatureIndicator
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateCloseIconButton
import com.warrenbrowse.vpn.lib.ui.component.dialog.InfoConfirmationDialog
import com.warrenbrowse.vpn.lib.ui.component.dialog.InfoConfirmationDialogTitleType
import com.warrenbrowse.vpn.lib.ui.component.drawVerticalScrollbar
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenCircularProgressIndicatorLarge
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.AlphaScrollbar
import com.warrenbrowse.vpn.lib.ui.theme.color.positive
import com.warrenbrowse.vpn.lib.ui.theme.color.warning
import org.koin.androidx.compose.koinViewModel
import org.koin.core.parameter.parametersOf

@Composable
fun SharedTransitionScope.SplitTunneling(
    isModal: Boolean,
    countryPerApp: Boolean,
    navigator: Navigator,
    animatedVisibilityScope: AnimatedVisibilityScope,
) {
    val viewModel = koinViewModel<SplitTunnelingViewModel> { parametersOf(isModal) }
    val state by viewModel.uiState.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val packageManager = remember(context) { context.packageManager }
    val leave = dropUnlessResumed { navigator.goBack() }
    val actions =
        remember(viewModel) {
            AppRoutingActions(
                onChooseDefault = viewModel::onChooseDefault,
                onOpenAddApp = viewModel::onOpenAddApp,
                onAddAppSearchChange = viewModel::onAddAppSearchChange,
                onShowSystemApps = viewModel::onShowSystemApps,
                onOpenApp = viewModel::onOpenApp,
                onChooseRoute = viewModel::onChooseRoute,
                onSetLocked = viewModel::onSetLocked,
                onOpenCountries = viewModel::onOpenCountries,
                onRemoveRule = viewModel::onRemoveRule,
                onDone = viewModel::onDone,
                onBack = { if (!viewModel.onBack()) leave() },
                picker =
                    CountryPickerActions(
                        onSearchChange = viewModel::onPickerSearchChange,
                        onToggleCountry = viewModel::onPickerToggleCountry,
                        onChoose = viewModel::onChooseExit,
                        onCancel = { viewModel.onBack() },
                    ),
                onConfirmNarrowing = viewModel::onConfirmNarrowing,
                onCancelNarrowing = viewModel::onCancelNarrowing,
            )
        }
    // Only a page opened over the list goes back inside the screen; the list leaves it.
    val page = state.contentOrNull()?.page
    BackHandler(enabled = page != null && page != AppRoutingPage.Rules) { viewModel.onBack() }

    SplitTunnelingScreen(
        state = state,
        actions = actions,
        modifier =
            Modifier.sharedBounds(
                rememberSharedContentState(
                    key =
                        if (countryPerApp) {
                            FeatureIndicator.APP_COUNTRIES
                        } else {
                            FeatureIndicator.SPLIT_TUNNELING
                        }
                ),
                animatedVisibilityScope = animatedVisibilityScope,
            ),
        onResolveIcon = { packageName -> packageManager.getApplicationIconOrNull(packageName) },
    )
}

@Composable
fun SplitTunnelingScreen(
    state: Lc<Loading, AppRoutingUiState>,
    actions: AppRoutingActions,
    onResolveIcon: (PackageName) -> Drawable?,
    modifier: Modifier = Modifier,
) {
    val content = state.contentOrNull()
    if (content == null) {
        RulesScaffold(isModal = state.isModal(), onBack = actions.onBack, modifier = modifier) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                WarrenCircularProgressIndicatorLarge()
            }
        }
        return
    }
    AnimatedContent(
        targetState = content.page,
        contentKey = { it::class },
        transitionSpec = { fadeIn() togetherWith fadeOut() },
        modifier = modifier,
        label = "app routing page",
    ) { page ->
        when (page) {
            AppRoutingPage.Rules -> RulesPage(content, actions, onResolveIcon)
            is AppRoutingPage.AddApp -> AddAppPage(page, actions, onResolveIcon)
            is AppRoutingPage.Route ->
                RoutePage(page, content.countrySupported, actions, onResolveIcon)
            is AppRoutingPage.Country -> CountryPickerPage(page.picker, actions.picker)
        }
    }
    content.confirmation?.let { NarrowingDialog(it, actions) }
}

@Composable
private fun RulesScaffold(
    isModal: Boolean,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
    content: @Composable (Modifier) -> Unit,
) {
    ScaffoldWithSmallTopBar(
        modifier = modifier.fillMaxSize(),
        appBarTitle = stringResource(id = R.string.app_routing),
        navigationIcon = {
            if (isModal) {
                NavigateCloseIconButton(onNavigateClose = onBack)
            } else {
                unlessIsDetail { NavigateBackIconButton(onNavigateBack = onBack) }
            }
        },
        content = content,
    )
}

/** The default route, then the rules, as the desktop shows them. */
@Composable
private fun RulesPage(
    state: AppRoutingUiState,
    actions: AppRoutingActions,
    onResolveIcon: (PackageName) -> Drawable?,
) {
    RulesScaffold(isModal = state.isModal, onBack = actions.onBack) { modifier ->
        val listState = rememberLazyListState()
        LazyColumn(
            modifier =
                modifier
                    .drawVerticalScrollbar(
                        state = listState,
                        color = MaterialTheme.colorScheme.onSurface.copy(alpha = AlphaScrollbar),
                    )
                    .background(MaterialTheme.colorScheme.surface)
                    .padding(horizontal = Dimens.sideMarginNew),
            state = listState,
            verticalArrangement = Arrangement.spacedBy(Dimens.smallPadding),
        ) {
            item(key = "subtitle") {
                Text(
                    text = stringResource(R.string.app_routing_description),
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(bottom = Dimens.mediumPadding),
                )
            }
            defaultRouteSection(state.defaultRoute, actions.onChooseDefault)
            rulesSection(state, actions, onResolveIcon)
            if (state.someAppOutside) {
                item(key = "lockdown") {
                    Text(
                        text = stringResource(R.string.app_routing_lockdown_note),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(top = Dimens.mediumPadding),
                    )
                }
            }
            item(key = "end") { Spacer(Modifier.height(Dimens.cellVerticalSpacing)) }
        }
    }
}

private fun LazyListScope.defaultRouteSection(
    defaultRoute: DefaultRoute,
    onChoose: (DefaultRoute) -> Unit,
) {
    item(key = "default_title") { SectionTitle(stringResource(R.string.app_routing_other_apps)) }
    item(key = "default_choice") { DefaultRouteChoice(defaultRoute, onChoose) }
    item(key = "default_help") {
        Text(
            text =
                stringResource(
                    when (defaultRoute) {
                        DefaultRoute.Vpn -> R.string.app_routing_default_vpn_help
                        DefaultRoute.Direct -> R.string.app_routing_default_direct_help
                    }
                ),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(bottom = Dimens.mediumPadding),
        )
    }
}

/** "Through the VPN" and "Outside the VPN", side by side, the chosen one filled. */
@Composable
private fun DefaultRouteChoice(defaultRoute: DefaultRoute, onChoose: (DefaultRoute) -> Unit) {
    Row(
        modifier =
            Modifier.fillMaxWidth()
                .height(IntrinsicSegmentHeight)
                .clip(CardShape)
                .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                .selectableGroup()
    ) {
        DefaultRoute.entries.forEach { route ->
            val selected = route == defaultRoute
            Box(
                modifier =
                    Modifier.weight(1f)
                        .fillMaxSize()
                        .background(
                            if (selected) MaterialTheme.colorScheme.positive
                            else MaterialTheme.colorScheme.surfaceContainerHigh
                        )
                        .selectable(
                            selected = selected,
                            role = Role.RadioButton,
                            onClick = { onChoose(route) },
                        )
                        .padding(horizontal = Dimens.smallPadding),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text =
                        stringResource(
                            when (route) {
                                DefaultRoute.Vpn -> R.string.app_route_through_vpn
                                DefaultRoute.Direct -> R.string.app_route_outside_vpn
                            }
                        ),
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = if (selected) FontWeight.Bold else FontWeight.Normal,
                    color = MaterialTheme.colorScheme.onSurface,
                    textAlign = TextAlign.Center,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

private fun LazyListScope.rulesSection(
    state: AppRoutingUiState,
    actions: AppRoutingActions,
    onResolveIcon: (PackageName) -> Drawable?,
) {
    item(key = "rules_title") {
        Row(
            modifier = Modifier.fillMaxWidth().padding(bottom = Dimens.tinyPadding),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Row(modifier = Modifier.weight(1f).semantics(mergeDescendants = true) { heading() }) {
                Text(
                    text = stringResource(R.string.app_routing_rules),
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                if (state.rules.isNotEmpty()) {
                    Text(
                        text = state.rules.size.toString(),
                        style = MaterialTheme.typography.titleMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(start = Dimens.smallPadding),
                    )
                }
            }
            AddAppPill(onClick = actions.onOpenAddApp)
        }
    }
    when {
        state.showNoRules ->
            item(key = "no_rules") {
                InfoCard(text = stringResource(R.string.app_routing_no_rules), warning = false)
            }
        state.rules.isEmpty() && state.fullTunnelFallback ->
            item(key = "fallback") {
                InfoCard(text = stringResource(R.string.app_routing_fallback_warning), warning = true)
            }
    }
    items(items = state.rules, key = { it.app.packageName.value }) { rule ->
        RuleRow(rule, onClick = { actions.onOpenApp(rule.app) }, onResolveIcon = onResolveIcon)
    }
}

@Composable
private fun SectionTitle(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.titleMedium,
        color = MaterialTheme.colorScheme.onSurface,
        modifier = Modifier.semantics { heading() },
    )
}

/** "+ App", which opens the apps without a rule. */
@Composable
private fun AddAppPill(onClick: () -> Unit) {
    val description = stringResource(R.string.app_routing_add_app)
    Row(
        modifier =
            Modifier.heightIn(min = PillHeight)
                .clip(CircleShape)
                .border(1.5.dp, outlineColor(), CircleShape)
                .clickable(onClick = onClick)
                .clearAndSetSemantics {
                    contentDescription = description
                    role = Role.Button
                }
                .padding(horizontal = Dimens.cellStartPadding - 2.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(Dimens.tinyPadding + 2.dp),
    ) {
        Icon(
            imageVector = Icons.Rounded.Add,
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.size(18.dp),
        )
        Text(
            text = stringResource(R.string.app_routing_add_app_pill),
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurface,
        )
    }
}

/** The empty list, or the ocre warning of what really happens with no app in the VPN. */
@Composable
private fun InfoCard(text: String, warning: Boolean) {
    Row(
        modifier =
            Modifier.fillMaxWidth()
                .clip(CardShape)
                .background(
                    if (warning) MaterialTheme.colorScheme.warning.copy(alpha = WarningFill)
                    else MaterialTheme.colorScheme.surfaceContainer
                )
                .padding(horizontal = Dimens.mediumPadding, vertical = Dimens.cellStartPadding - 2.dp)
                .semantics(mergeDescendants = true) {},
        horizontalArrangement = Arrangement.spacedBy(Dimens.smallPadding + 2.dp),
    ) {
        if (warning) {
            Icon(
                imageVector = Icons.Rounded.WarningAmber,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.warning,
                modifier = Modifier.size(20.dp),
            )
        }
        Text(
            text = text,
            style = MaterialTheme.typography.bodyMedium,
            color =
                if (warning) MaterialTheme.colorScheme.warning
                else MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

/** `[icon] name / route line ...... [route chip]`; the whole row opens the app's route. */
@Composable
private fun RuleRow(
    rule: AppRuleItem,
    onClick: () -> Unit,
    onResolveIcon: (PackageName) -> Drawable?,
) {
    Row(
        modifier =
            Modifier.fillMaxWidth()
                .heightIn(min = RowMinHeight)
                .clip(CardShape)
                .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                .clickable(onClick = onClick)
                .semantics(mergeDescendants = true) { role = Role.Button }
                .padding(horizontal = Dimens.cellStartPadding - 4.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(Dimens.cellStartPadding - 4.dp),
    ) {
        AppIcon(rule.app.packageName, onResolveIcon, AppIconSize)
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = rule.app.name,
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurface,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            rule.line?.let { RouteStatusLine(it) }
        }
        RouteChip(rule.route, locked = rule.locked)
    }
}

/**
 * Asked before a change makes a few apps the only ones in the VPN where every app uses it now
 * (docs/app-routing.md section 3.4); cancelling leaves the settings as they were.
 */
@Composable
private fun NarrowingDialog(confirmation: NarrowingConfirmation, actions: AppRoutingActions) {
    val app = confirmation.app
    InfoConfirmationDialog(
        onResult = { confirmed ->
            if (confirmed != null) actions.onConfirmNarrowing() else actions.onCancelNarrowing()
        },
        titleType =
            if (app != null) {
                InfoConfirmationDialogTitleType.IconAndTitle(
                    stringResource(R.string.app_country_only_app_title, app.name)
                )
            } else {
                InfoConfirmationDialogTitleType.IconOnly
            },
        confirmButtonTitle = stringResource(R.string.cont),
        cancelButtonTitle = stringResource(R.string.cancel),
    ) {
        Text(
            text =
                stringResource(
                    if (app != null) R.string.app_route_only_app
                    else R.string.app_routing_default_direct_confirm
                ),
            color = MaterialTheme.colorScheme.onSurface,
            style = MaterialTheme.typography.bodySmall,
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

private fun Lc<Loading, AppRoutingUiState>.isModal(): Boolean =
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

private const val WarningFill = 0.14f
private val IntrinsicSegmentHeight = 48.dp
private val PillHeight = 40.dp
private val RowMinHeight = 60.dp
