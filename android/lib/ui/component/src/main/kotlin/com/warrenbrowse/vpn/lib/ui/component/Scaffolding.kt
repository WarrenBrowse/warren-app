package com.warrenbrowse.vpn.lib.ui.component

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FabPosition
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.graphics.Color
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenSnackbar

@Composable
fun ScaffoldWithTopBar(
    topBarColor: Color,
    modifier: Modifier = Modifier,
    iconTintColor: Color = MaterialTheme.colorScheme.onPrimary,
    onSettingsClicked: (() -> Unit)?,
    onAccountClicked: (() -> Unit)?,
    forumSlot: ForumHeaderSlot? = null,
    onForumClicked: (() -> Unit)? = null,
    isIconAndLogoVisible: Boolean = true,
    accountShortPubkey: String? = null,
    accountTimeLeft: String? = null,
    onCopyPubkey: (() -> Unit)? = null,
    snackbarHostState: SnackbarHostState = remember { SnackbarHostState() },
    enabled: Boolean = true,
    navigationIcon: @Composable () -> Unit = {},
    content: @Composable (PaddingValues) -> Unit,
) {
    Scaffold(
        modifier = modifier,
        topBar = {
            Column {
                WarrenTopBar(
                    containerColor = topBarColor,
                    iconTintColor = iconTintColor,
                    onSettingsClicked = onSettingsClicked,
                    onAccountClicked = onAccountClicked,
                    forumSlot = forumSlot,
                    onForumClicked = onForumClicked,
                    isIconAndLogoVisible = isIconAndLogoVisible,
                    enabled = enabled,
                    navigationIcon = navigationIcon,
                )
                // Desktop AppMainHeader second row: pubkey (copyable) + time left.
                if (accountShortPubkey != null || accountTimeLeft != null) {
                    WarrenMainHeaderSubRow(
                        containerColor = topBarColor,
                        tintColor = iconTintColor,
                        shortPubkey = accountShortPubkey,
                        timeLeft = accountTimeLeft,
                        onCopyPubkey = onCopyPubkey,
                    )
                }
            }
        },
        snackbarHost = {
            SnackbarHost(
                snackbarHostState,
                snackbar = { snackbarData -> WarrenSnackbar(snackbarData = snackbarData) },
            )
        },
        content = content,
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ScaffoldWithSmallTopBar(
    appBarTitle: String,
    modifier: Modifier = Modifier,
    navigationIcon: @Composable () -> Unit = {},
    actions: @Composable RowScope.() -> Unit = {},
    snackbarHostState: SnackbarHostState = remember { SnackbarHostState() },
    bottomBar: @Composable () -> Unit = {},
    floatingActionButton: @Composable () -> Unit = {},
    floatingActionButtonPosition: FabPosition = FabPosition.End,
    content: @Composable (modifier: Modifier) -> Unit,
) {
    // Material's scroll-linked container transition, which the bar had no
    // counterpart for: pinned rather than enterAlways, because these screens
    // carry the only way back and a bar that scrolls away takes it with it.
    val scrollBehavior = TopAppBarDefaults.pinnedScrollBehavior()
    Scaffold(
        modifier =
            modifier
                .fillMaxSize()
                .imePadding()
                .nestedScroll(scrollBehavior.nestedScrollConnection),
        topBar = {
            WarrenSmallTopBar(
                title = appBarTitle,
                navigationIcon = navigationIcon,
                actions = actions,
                scrollBehavior = scrollBehavior,
            )
        },
        snackbarHost = {
            SnackbarHost(
                snackbarHostState,
                snackbar = { snackbarData -> WarrenSnackbar(snackbarData = snackbarData) },
            )
        },
        bottomBar = bottomBar,
        floatingActionButton = floatingActionButton,
        floatingActionButtonPosition = floatingActionButtonPosition,
        content = { content(Modifier.fillMaxSize().padding(it)) },
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ScaffoldWithSmallTopBar(
    appBarTitle: String,
    modifier: Modifier = Modifier,
    navigationIcon: @Composable () -> Unit = {},
    actions: @Composable RowScope.() -> Unit = {},
    snackbarHostState: SnackbarHostState = remember { SnackbarHostState() },
    content: @Composable (modifier: Modifier) -> Unit,
) {
    val scrollBehavior = TopAppBarDefaults.pinnedScrollBehavior()
    Scaffold(
        modifier =
            modifier
                .fillMaxSize()
                .imePadding()
                .nestedScroll(scrollBehavior.nestedScrollConnection),
        topBar = {
            WarrenSmallTopBar(
                title = appBarTitle,
                navigationIcon = navigationIcon,
                actions = actions,
                scrollBehavior = scrollBehavior,
            )
        },
        snackbarHost = {
            SnackbarHost(
                snackbarHostState,
                snackbar = { snackbarData -> WarrenSnackbar(snackbarData = snackbarData) },
            )
        },
        content = { content(Modifier.fillMaxSize().padding(it)) },
    )
}
