package com.warrenbrowse.vpn.common.compose

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.Build
import android.os.PersistableBundle
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.toClipEntry
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

typealias CopyToClipboardHandle = (content: String, toastMessage: String?) -> Unit

private const val IS_SENSITIVE_FLAG = "android.content.extra.IS_SENSITIVE"

/** Desktop `CLIPBOARD_CLEAR_DURATION`: a copied recovery phrase lives one minute. */
const val MNEMONIC_CLIPBOARD_CLEAR_MS = 60_000L

@Composable
fun createCopyToClipboardHandle(
    snackbarHostState: SnackbarHostState = SnackbarHostState(),
    isSensitive: Boolean,
    // When set, the clip is dropped again after this delay unless something
    // else has taken the clipboard over meanwhile. Use it for secrets: the
    // sensitive flag only hides the preview, the clip itself would otherwise
    // stay readable by keyboard clipboard history until overwritten.
    autoClearAfterMs: Long? = null,
): CopyToClipboardHandle {
    val scope = rememberCoroutineScope()
    val clipboardManager = LocalClipboard.current
    // The application context: the clear outlives the screen that copied, so
    // holding an Activity for the whole delay would be a leak.
    val appContext = LocalContext.current.applicationContext

    return { textToCopy: String, toastMessage: String? ->
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU && toastMessage != null) {
            scope.launch {
                snackbarHostState.showSnackbarImmediately(
                    message = toastMessage,
                    duration = SnackbarDuration.Short,
                )
            }
        }

        scope.launch {
            val clip =
                ClipData.newPlainText("", textToCopy)
                    .apply {
                        description.extras =
                            PersistableBundle().apply { putBoolean(IS_SENSITIVE_FLAG, isSensitive) }
                    }
                    .toClipEntry()

            clipboardManager.setClipEntry(clip)
            autoClearAfterMs?.let { ClipboardAutoClear.schedule(appContext, textToCopy, it) }
        }
    }
}

/**
 * Compare-and-clear rule for a scheduled clipboard wipe: the clip is only
 * dropped when it still holds exactly what was copied.
 *
 * A clip that reads back as `null` is NOT proof the secret is gone: reading the
 * primary clip from the background answers `null` from Android 10 on. Clearing
 * on that would throw away whatever another app has since put there.
 */
internal fun shouldClearClipboard(currentClipText: String?, copiedText: String): Boolean =
    currentClipText == copiedText

/**
 * Process-scoped scheduler for the delayed clipboard wipe.
 *
 * The wipe deliberately does not hang off the copying screen's composition
 * scope: the user usually leaves the backup screen long before the minute is
 * up, and a cancelled timer would leave the recovery phrase on the clipboard,
 * which is the whole thing this exists to prevent. One pending wipe at a time
 * is enough, a second copy replaces the first.
 */
private object ClipboardAutoClear {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private var pending: Job? = null

    fun schedule(context: Context, copiedText: String, afterMs: Long) {
        pending?.cancel()
        pending =
            scope.launch {
                delay(afterMs)
                val manager =
                    context.getSystemService(ClipboardManager::class.java) ?: return@launch
                val current =
                    manager.primaryClip
                        ?.takeIf { it.itemCount > 0 }
                        ?.getItemAt(0)
                        ?.text
                        ?.toString()
                if (shouldClearClipboard(current, copiedText)) {
                    manager.clearPrimaryClip()
                }
            }
    }
}
