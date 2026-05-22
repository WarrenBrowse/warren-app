package com.warrenbrowse.vpn.common.compose

import androidx.compose.ui.platform.UriHandler
import arrow.core.Either
import co.touchlab.kermit.Logger

// D.4 step 46: createOpenAccountPageHook removed — only consumer was the
// OpenAccountManagementPageInBrowser side effect (deleted in step 43).

fun UriHandler.createUriHook(uri: String): () -> Unit = { safeOpenUri(uri) }

fun UriHandler.safeOpenUri(uri: String): Either<IllegalArgumentException, Unit> =
    try {
        Either.Right(openUri(uri))
    } catch (e: IllegalArgumentException) {
        // E.g user has no browser or invalid uri
        Logger.e("Failed to open uri: $uri", e)
        Either.Left(e)
    }
