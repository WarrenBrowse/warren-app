package com.warrenbrowse.vpn.feature.settings.impl.support

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Share
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.common.compose.unlessIsDetail
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** Largest slice of the report shown; the file itself is sent whole. */
private const val PREVIEW_MAX_CHARS = 400_000

/**
 * The exact redacted report about to be sent, so the user can check what
 * leaves the device (the desktop "View the logs"). Read-only; the send stays
 * on the form behind it. The share action is the last resort when the broker
 * itself cannot be reached: the same file, handed to whatever app the user
 * picks.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ReportPreview(navigator: Navigator, path: String) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val shareTitle = stringResource(R.string.report_problem_share)
    var text by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(path) {
        text =
            withContext(Dispatchers.IO) {
                runCatching {
                    val content = File(path).readText()
                    if (content.length > PREVIEW_MAX_CHARS) {
                        content.take(PREVIEW_MAX_CHARS) + "\n[preview truncated]"
                    } else {
                        content
                    }
                }
                    .getOrNull()
            }
                ?: ""
    }

    ScaffoldWithSmallTopBar(
        appBarTitle = stringResource(R.string.report_problem_preview_title),
        navigationIcon = {
            unlessIsDetail {
                NavigateBackIconButton(onNavigateBack = dropUnlessResumed { navigator.goBack() })
            }
        },
        actions = {
            // The share copies the report (up to tens of MB) before the sheet
            // opens, so the copy runs off the main thread.
            IconButton(
                onClick = {
                    scope.launch {
                        val intent =
                            withContext(Dispatchers.IO) { reportShareIntent(context, path, shareTitle) }
                        context.startActivity(intent)
                    }
                },
                enabled = text != null,
            ) {
                Icon(imageVector = Icons.Rounded.Share, contentDescription = shareTitle)
            }
        },
    ) { modifier ->
        Column(
            modifier =
                Modifier.fillMaxSize()
                    .then(modifier)
                    .verticalScroll(rememberScrollState())
                    .horizontalScroll(rememberScrollState())
                    .padding(horizontal = Dimens.sideMargin, vertical = Dimens.mediumPadding)
        ) {
            Text(
                text = text ?: stringResource(R.string.report_problem_collecting),
                style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
                softWrap = false,
            )
        }
    }
}
