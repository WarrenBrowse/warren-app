package com.warrenbrowse.vpn.common.compose

import androidx.compose.ui.platform.UriHandler
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class UriHandlerExtensionsTest {

    private fun refusingWith(error: Exception) =
        object : UriHandler {
            override fun openUri(uri: String) = throw error
        }

    @Test
    fun `a link no app can open is reported instead of crashing the app`() {
        val result =
            refusingWith(IllegalArgumentException("no activity")).safeOpenUri(LINK)

        assertTrue(result.isLeft())
    }

    @Test
    fun `a link the platform refuses to open is reported instead of crashing the app`() {
        // Some Android 9 to 11 builds throw SecurityException when they refuse
        // to start the activity for a link.
        val result = refusingWith(SecurityException("refused")).safeOpenUri(LINK)

        assertTrue(result.isLeft())
    }

    private companion object {
        const val LINK = "https://beta.warren.ro/faq"
    }
}
