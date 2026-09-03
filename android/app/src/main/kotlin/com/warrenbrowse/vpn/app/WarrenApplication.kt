package com.warrenbrowse.vpn.app

import android.app.Application
import android.os.StrictMode
import android.os.strictmode.UntaggedSocketViolation
import androidx.compose.runtime.Composer
import androidx.compose.runtime.ExperimentalComposeRuntimeApi
import androidx.compose.runtime.tooling.ComposeStackTraceMode
import co.touchlab.kermit.Logger
import co.touchlab.kermit.Severity
import java.io.IOException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.asExecutor
import kotlinx.coroutines.launch
import com.warrenbrowse.vpn.BuildConfig
import com.warrenbrowse.vpn.app.util.FileLogWriter
import com.warrenbrowse.vpn.di.ApplicationScope
import com.warrenbrowse.vpn.di.KERMIT_FILE_LOG_DIR_NAME
import com.warrenbrowse.vpn.di.appModule
import com.warrenbrowse.vpn.jni.WarrenJni
import com.warrenbrowse.vpn.lib.pushnotification.NotificationChannelFactory
import com.warrenbrowse.vpn.lib.pushnotification.NotificationManager
import org.koin.android.ext.android.getKoin
import org.koin.android.ext.koin.androidContext
import org.koin.core.context.loadKoinModules
import org.koin.core.context.startKoin

private const val LOG_TAG = "warren"

@OptIn(ExperimentalComposeRuntimeApi::class)
class WarrenApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        Logger.setTag(LOG_TAG)
        if (!BuildConfig.DEBUG) {
            Logger.setMinSeverity(Severity.Info)
        }
        // The Rust runtime and its log file exist from the first instant of
        // the process, not from the first VPN service: a forum deep link that
        // cold-starts the app signs through that runtime, and the failure it
        // may meet is written to a file the user can send.
        try {
            WarrenJni.initLogger(filesDir.absolutePath)
        } catch (e: RuntimeException) {
            Logger.e(throwable = e) { "Rust logger init failed" }
        }
        if (BuildConfig.DEBUG) {
            // Improve compose stack traces
            // Comes with a performance penalty, so only enable in debug builds
            Composer.setDiagnosticStackTraceMode(ComposeStackTraceMode.SourceInformation)
            enableStrictMode()
        }
        startKoin { androidContext(this@WarrenApplication) }
        loadKoinModules(listOf(appModule))
        with(getKoin()) {
            get<NotificationChannelFactory>()
            get<NotificationManager>()
            initFileLogger(get<ApplicationScope>())
        }
    }

    private fun initFileLogger(scope: CoroutineScope) {
        scope.launch(Dispatchers.IO) {
            try {
                val fileLogWriter =
                    FileLogWriter(
                        logDir = filesDir.toPath().resolve(KERMIT_FILE_LOG_DIR_NAME),
                        scope = scope,
                    )
                Logger.addLogWriter(fileLogWriter)
            } catch (e: IOException) { // This shouldn't happen but just in case catch here.
                Logger.e("Failed to initialize file log writer", e)
            }
        }
    }

    private fun enableStrictMode() {
        val executor = Dispatchers.Default.asExecutor()

        StrictMode.setThreadPolicy(
            StrictMode.ThreadPolicy.Builder()
                .detectAll()
                .penaltyListener(executor) { violation ->
                    android.util.Log.e(
                        "StrictMode",
                        "StrictMode thread policy violation:",
                        violation,
                    )
                }
                .build()
        )

        StrictMode.setVmPolicy(
            StrictMode.VmPolicy.Builder()
                .detectAll()
                .penaltyListener(executor) { violation ->
                    // Filter out violations that we don't care about that would spam the logs
                    if (violation is UntaggedSocketViolation) {
                        return@penaltyListener
                    }
                    android.util.Log.e("StrictMode", "StrictMode VM policy violation:", violation)
                }
                .build()
        )
    }
}
