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
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenSnackbar

@Composable
fun ScaffoldWithTopBar(
    topBarColor: Color,
    modifier: Modifier = Modifier,
    iconTintColor: Color = MaterialTheme.colorScheme.onPrimary,
    onSettingsClicked: (() -> Unit)?,
    onAccountClicked: (() -> Unit)?,
    isIconAndLogoVisible: Boolean = true,
    snackbarHostState: SnackbarHostState = remember { SnackbarHostState() },
    enabled: Boolean = true,
    content: @Composable (PaddingValues) -> Unit,
) {
    Scaffold(
        modifier = modifier,
        topBar = {
            WarrenTopBar(
                containerColor = topBarColor,
                iconTintColor = iconTintColor,
                onSettingsClicked = onSettingsClicked,
                onAccountClicked = onAccountClicked,
                isIconAndLogoVisible = isIconAndLogoVisible,
                enabled = enabled,
            )
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

@Composable
fun ScaffoldWithTopBarAndDeviceName(
    topBarColor: Color,
    modifier: Modifier = Modifier,
    iconTintColor: Color = MaterialTheme.colorScheme.onPrimary,
    onSettingsClicked: (() -> Unit)?,
    onAccountClicked: (() -> Unit)?,
    isIconAndLogoVisible: Boolean = true,
    snackbarHostState: SnackbarHostState = remember { SnackbarHostState() },
    deviceName: String?,
    timeLeft: Long?,
    content: @Composable (PaddingValues) -> Unit,
) {
    Scaffold(
        modifier = modifier,
        topBar = {
            Column {
                WarrenTopBarWithDeviceName(
                    containerColor = topBarColor,
                    iconTintColor = iconTintColor,
                    onSettingsClicked = onSettingsClicked,
                    onAccountClicked = onAccountClicked,
                    isIconAndLogoVisible = isIconAndLogoVisible,
                    deviceName = deviceName,
                    daysLeftUntilExpiry = timeLeft,
                )
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
    Scaffold(
        modifier = modifier.fillMaxSize().imePadding(),
        topBar = {
            WarrenSmallTopBar(
                title = appBarTitle,
                navigationIcon = navigationIcon,
                actions = actions,
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
    Scaffold(
        modifier = modifier.fillMaxSize().imePadding(),
        topBar = {
            WarrenSmallTopBar(
                title = appBarTitle,
                navigationIcon = navigationIcon,
                actions = actions,
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
