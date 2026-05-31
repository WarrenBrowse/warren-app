package com.warrenbrowse.vpn.screen.onboarding

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.login.api.WarrenWalletNavKey
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.ui.resource.R
import org.koin.compose.koinInject

/**
 * First-launch onboarding welcome. Introduces Warren's value props, then
 * hands off to wallet creation. Gated to be shown once (see
 * [com.warrenbrowse.vpn.screen.splash.SplashViewModel]); tapping "Get
 * started" marks it completed and navigates to the wallet flow.
 */
@Composable
fun OnboardingScreen(navigator: Navigator) {
    val settings = koinInject<WarrenLocalSettingsRepository>()

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        Image(
            painter = painterResource(id = R.drawable.logo_icon),
            contentDescription = null,
            modifier = Modifier.size(96.dp).padding(top = 24.dp),
        )
        Text(
            text = "Welcome to Warren",
            style = MaterialTheme.typography.headlineSmall,
            color = MaterialTheme.colorScheme.onSurface,
        )
        Text(
            text = "A fast, private VPN built on the Warren QUIC tunnel. Your " +
                "identity is a wallet you alone control — no email, no account number.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        ValueProp(
            title = "Always-on obfuscation",
            body = "Your traffic looks like ordinary HTTPS browsing (HTTP/3 mimicry).",
        )
        ValueProp(
            title = "No logs, no tracking",
            body = "We can't tie traffic to you — there's no account identity to log.",
        )
        ValueProp(
            title = "You hold the keys",
            body = "A 12-word recovery phrase is your only credential. Back it up safely.",
        )

        Button(
            modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
            onClick = {
                settings.setOnboardingCompleted(true)
                navigator.navigate(WarrenWalletNavKey, clearBackStack = true)
            },
        ) { Text("Get started") }
    }
}

@Composable
private fun ValueProp(title: String, body: String) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(
            text = "•",
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.primary,
        )
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                text = title,
                style = MaterialTheme.typography.titleSmall,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Text(
                text = body,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}
