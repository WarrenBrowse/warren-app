package com.warrenbrowse.vpn.feature.language.impl

import android.content.Context
import androidx.appcompat.app.AppCompatDelegate
import androidx.core.os.LocaleListCompat
import java.util.Locale
import com.warrenbrowse.vpn.lib.ui.resource.R

/**
 * Per-app language through AppCompat rather than the framework `LocaleManager`.
 *
 * The framework API lands only on API 33, which left every older device with no
 * language picker at all. AppCompat applies the same choice on those releases
 * and delegates to `LocaleManager` from 33 up, so behaviour above the line is
 * unchanged and the row can be offered unconditionally. Persistence below 33 is
 * AppCompat's, enabled by the `autoStoreLocales` metadata in the app manifest.
 */
class LanguageRepository(private val context: Context) {

    fun getSupportedLocales(): List<Locale> =
        supportedLocalesFromTags(readLocaleConfigTags(context.resources, R.xml.locales_config))

    fun getAppLocale(): Locale? =
        AppCompatDelegate.getApplicationLocales().takeIf { !it.isEmpty }?.get(0)

    fun setAppLocale(locale: Locale?) {
        AppCompatDelegate.setApplicationLocales(
            if (locale == null) {
                LocaleListCompat.getEmptyLocaleList()
            } else {
                LocaleListCompat.forLanguageTags(locale.toLanguageTag())
            }
        )
    }
}
