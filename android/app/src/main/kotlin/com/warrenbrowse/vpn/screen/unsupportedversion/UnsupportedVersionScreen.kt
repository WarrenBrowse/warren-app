package com.warrenbrowse.vpn.screen.unsupportedversion

import android.content.ActivityNotFoundException
import android.content.Intent
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.tooling.preview.Preview
import androidx.core.app.ActivityCompat.finishAffinity
import androidx.core.net.toUri
import com.warrenbrowse.vpn.R
import com.warrenbrowse.vpn.feature.applisting.api.ResolveAppListingUseCase
import com.warrenbrowse.vpn.lib.ui.component.WarrenWordmarkLockup
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.designsystem.VariantButton
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.screen.nodaemon.getActivity
import org.koin.compose.koinInject

/**
 * Forced-update gate. Shown (replacing the whole UI) when the daemon-less mobile
 * version check reports the running app is no longer supported. There is no way
 * back into the app: the only actions are "Update now" (opens the store / the
 * download page, resolved the same way as the unsupported-version banner) and
 * "Quit". Mirrors the desktop `BlockingUpdateGate`.
 */
@Composable
fun UnsupportedVersionBlocked() {
    val context = LocalContext.current
    val resolveAppListing = koinInject<ResolveAppListingUseCase>()

    val quit = { finishAffinity(context.getActivity()!!) }

    // Hard block: the back gesture quits rather than dismissing the gate.
    BackHandler { quit() }

    UnsupportedVersionContent(
        onUpdate = {
            val target = resolveAppListing()
            try {
                context.startActivity(Intent(Intent.ACTION_VIEW, target.listingUri.toUri()))
            } catch (_: ActivityNotFoundException) {
                // No store / browser to handle the link; nothing else the gate can do.
            }
        },
        onQuit = quit,
    )
}

@Composable
private fun UnsupportedVersionContent(onUpdate: () -> Unit, onQuit: () -> Unit) {
    val backgroundColor = MaterialTheme.colorScheme.primary

    Box(
        contentAlignment = Alignment.Center,
        modifier =
            Modifier.background(backgroundColor).fillMaxSize().padding(horizontal = Dimens.sideMargin),
    ) {
        Column(
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            WarrenWordmarkLockup(
                color = MaterialTheme.colorScheme.onPrimary,
                height = Dimens.splashLockupHeight,
            )
            Text(
                text = stringResource(id = R.string.update_required_title),
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.onPrimary,
                textAlign = TextAlign.Center,
                modifier = Modifier.padding(top = Dimens.mediumPadding),
            )
            Text(
                text = stringResource(id = R.string.update_required_description),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
                modifier = Modifier.padding(top = Dimens.mediumPadding),
            )
            VariantButton(
                onClick = onUpdate,
                text = stringResource(id = R.string.update_now),
                modifier = Modifier.fillMaxWidth().padding(top = Dimens.mediumPadding),
            )
            PrimaryButton(
                onClick = onQuit,
                text = stringResource(id = R.string.quit),
                modifier = Modifier.fillMaxWidth().padding(top = Dimens.mediumPadding),
            )
        }
    }
}

@Preview
@Composable
private fun PreviewUnsupportedVersion() {
    AppTheme { UnsupportedVersionContent(onUpdate = {}, onQuit = {}) }
}
