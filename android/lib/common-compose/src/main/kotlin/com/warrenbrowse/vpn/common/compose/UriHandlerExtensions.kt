package com.warrenbrowse.vpn.common.compose

import androidx.compose.ui.platform.UriHandler
import arrow.core.Either
import co.touchlab.kermit.Logger

fun UriHandler.createUriHook(uri: String): () -> Unit = { safeOpenUri(uri) }

fun UriHandler.safeOpenUri(uri: String): Either<Exception, Unit> =
    try {
        Either.Right(openUri(uri))
    } catch (e: IllegalArgumentException) {
        // E.g user has no browser or invalid uri
        Logger.e("Failed to open a link", e)
        Either.Left(e)
    } catch (e: SecurityException) {
        // Some Android 9 to 11 builds throw this when the platform refuses to start the activity
        // for a link, which would otherwise crash the app.
        Logger.e("The platform refused to open a link", e)
        Either.Left(e)
    }
