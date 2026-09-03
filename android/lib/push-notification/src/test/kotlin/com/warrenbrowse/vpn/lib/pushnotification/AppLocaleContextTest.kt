package com.warrenbrowse.vpn.lib.pushnotification

import android.content.Context
import androidx.appcompat.app.AppCompatDelegate
import androidx.core.os.LocaleListCompat
import io.mockk.every
import io.mockk.mockk
import io.mockk.mockkStatic
import io.mockk.unmockkStatic
import io.mockk.verify
import kotlin.test.assertSame
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test

/**
 * The notification context. The branch that matters here is the default one:
 * every install that never touched the picker builds its notifications from
 * the context it always did, so the foreground service cannot be broken by a
 * configuration context nobody asked for.
 */
class AppLocaleContextTest {

    @BeforeEach
    fun mockDelegate() {
        mockkStatic(AppCompatDelegate::class)
    }

    @AfterEach
    fun unmockDelegate() {
        unmockkStatic(AppCompatDelegate::class)
    }

    @Test
    fun no_chosen_language_leaves_the_context_exactly_as_it_was() {
        every { AppCompatDelegate.getApplicationLocales() } returns
            LocaleListCompat.getEmptyLocaleList()
        val context = mockk<Context>()

        assertSame(context, context.withAppLocale())

        verify(exactly = 0) { context.createConfigurationContext(any()) }
    }
}
