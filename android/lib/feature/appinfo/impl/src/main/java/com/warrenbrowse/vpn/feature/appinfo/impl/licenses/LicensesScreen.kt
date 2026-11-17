package com.warrenbrowse.vpn.feature.appinfo.impl.licenses

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.common.compose.safeOpenUri
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.appinfo.api.LicensesNavKey
import com.warrenbrowse.vpn.lib.repository.WarrenProductFlags
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.component.drawVerticalScrollbar
import com.warrenbrowse.vpn.lib.ui.component.listitem.ExternalLinkListItem
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.AlphaScrollbar
import org.koin.compose.koinInject

@Composable
fun Licenses(navArgs: LicensesNavKey, navigator: Navigator) {
    val productFlags = koinInject<WarrenProductFlags>()
    val uriHandler = LocalUriHandler.current
    val resources = LocalContext.current.resources
    val licenseText = remember {
        formatLicenseText(
            resources.openRawResource(R.raw.gpl_3_0).bufferedReader().use { it.readText() }
        )
    }
    val appSource = appSourceUrl(navArgs.version, productFlags.isBeta)

    LicensesScreen(
        licenseText = licenseText,
        onBackClick = dropUnlessResumed { navigator.goBack() },
        openAppSource = { uriHandler.safeOpenUri(appSource) },
        openEngineSource = { uriHandler.safeOpenUri(ENGINE_SOURCE_URL) },
    )
}

@Composable
fun LicensesScreen(
    licenseText: String,
    onBackClick: () -> Unit,
    openAppSource: () -> Unit,
    openEngineSource: () -> Unit,
) {
    ScaffoldWithSmallTopBar(
        appBarTitle = stringResource(id = R.string.licenses_title),
        navigationIcon = { NavigateBackIconButton(onNavigateBack = onBackClick) },
    ) { modifier ->
        val scrollState = rememberScrollState()
        Column(
            modifier =
                modifier
                    .drawVerticalScrollbar(
                        scrollState,
                        MaterialTheme.colorScheme.onSurface.copy(alpha = AlphaScrollbar),
                    )
                    .verticalScroll(scrollState)
                    .padding(horizontal = Dimens.sideMarginNew),
            verticalArrangement = Arrangement.spacedBy(Dimens.mediumPadding),
        ) {
            listOf(R.string.licenses_app, R.string.licenses_upstream, R.string.licenses_engine)
                .forEach { paragraph ->
                    Text(
                        text = stringResource(paragraph),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurface,
                    )
                }
            Column(modifier = Modifier.fillMaxWidth()) {
                ExternalLinkListItem(
                    title = stringResource(R.string.licenses_app_source),
                    position = Position.Top,
                    onClick = openAppSource,
                )
                ExternalLinkListItem(
                    title = stringResource(R.string.licenses_engine_source),
                    position = Position.Bottom,
                    onClick = openEngineSource,
                )
            }
            Text(
                text = licenseText,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(bottom = Dimens.mediumPadding),
            )
        }
    }
}

@Preview
@Composable
private fun PreviewLicensesScreen() {
    AppTheme {
        LicensesScreen(
            licenseText = "GNU GENERAL PUBLIC LICENSE\n\nVersion 3, 29 June 2007",
            onBackClick = {},
            openAppSource = {},
            openEngineSource = {},
        )
    }
}
