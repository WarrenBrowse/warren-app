package com.warrenbrowse.vpn.feature.settings.impl

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
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
import androidx.compose.ui.unit.dp
import androidx.fragment.app.FragmentActivity
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.settings.api.SettingsNavKey
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.ui.component.ScaffoldWithSmallTopBar
import com.warrenbrowse.vpn.lib.ui.component.button.NavigateBackIconButton
import kotlinx.coroutines.launch
import org.koin.compose.koinInject

/**
 * D.5 wallet settings host screen. Wraps [WarrenWalletSettingsSection]
 * inside the standard settings scaffold and threads the host
 * [FragmentActivity] required by `BiometricPrompt`.
 *
 * Reached via [com.warrenbrowse.vpn.feature.settings.api.WarrenWalletSettingsNavKey]
 * from the main Settings screen ("Wallet" entry).
 *
 * Hosts a "Warren Connect (test)" button (D.4 step 7 follow-up) that
 * dispatches the end-to-end Quinn connect flow via the
 * `WarrenQuinnConnectInvoker` interface (registered in app/AppModule
 * pointing at `WarrenConnectUseCase`). The button lives here, behind
 * the wallet settings, so we can exercise the connect path without
 * having to refactor the main Connect button's wiring across the
 * lib/feature/home boundary.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WarrenWalletSettings(navigator: Navigator) {
    val activity = LocalContext.current as FragmentActivity
    val walletRepository = koinInject<WalletRepository>()
    val quinnConnect = koinInject<WarrenQuinnConnectInvoker>()
    val scope = rememberCoroutineScope()
    var connectStatus by remember { mutableStateOf<String?>(null) }

    ScaffoldWithSmallTopBar(
        appBarTitle = "Wallet",
        navigationIcon = {
            NavigateBackIconButton(onNavigateBack = {
                navigator.goBackUntil(SettingsNavKey)
            })
        },
    ) { modifier ->
        Column(
            modifier = Modifier.fillMaxSize().then(modifier),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            WarrenWalletSettingsSection(
                activity = activity,
                walletRepository = walletRepository,
            )

            // Test button: dispatch end-to-end Quinn connect via the
            // app-side use-case. Surfaces the result as inline text;
            // a Snackbar would be nicer once the Connect button proper
            // gets wired in lib/feature/home/impl.
            OutlinedButton(
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                onClick = {
                    scope.launch {
                        connectStatus = "Connecting..."
                        val message = try {
                            quinnConnect.connect(activity)
                        } catch (e: Exception) {
                            Logger.e(throwable = e) { "Warren Connect invocation failed" }
                            "Connect failed: ${e.message}"
                        }
                        connectStatus = message
                    }
                },
            ) { Text("Warren Connect (test)") }

            connectStatus?.let { msg ->
                LaunchedEffect(msg) { Logger.i("Warren Connect status: $msg") }
                Text(
                    text = msg,
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(horizontal = 16.dp),
                )
            }
        }
    }
}

/**
 * Lib-module-facing surface for the Warren Connect use-case. Defined
 * here in `lib/feature/settings/impl` (rather than pulled in from app)
 * to keep the dependency arrow correct: features may not depend on
 * `app`. The implementation lives in `app/connect/WarrenConnectUseCase`
 * and is bound to this interface in `di/AppModule`.
 */
interface WarrenQuinnConnectInvoker {
    suspend fun connect(activity: FragmentActivity): String
}
