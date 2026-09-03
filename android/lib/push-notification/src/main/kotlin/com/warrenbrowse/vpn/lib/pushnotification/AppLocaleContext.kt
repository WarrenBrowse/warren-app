package com.warrenbrowse.vpn.lib.pushnotification

import android.content.Context
import android.content.res.Configuration
import android.os.LocaleList
import androidx.appcompat.app.AppCompatDelegate

/**
 * The same context, resolving strings in the language the reader picked.
 *
 * Below API 33 the per-app language is AppCompat's own, and it reaches
 * `AppCompatActivity` contexts alone: the framework `LocaleManager` does not
 * exist there, so the application context and the VPN service keep resolving
 * in the SYSTEM language. A notification built from one of those stays in the
 * system language while the app itself is in the chosen one, and those
 * releases (minSdk 28 to 32) are exactly the ones the unconditional picker
 * exists to serve. From API 33 up the framework applies the choice
 * process-wide, so wrapping again resolves the same strings.
 *
 * Read on every notification rather than cached: the choice can change while
 * the service is running, and a cached context would keep the old language
 * until the next process start.
 */
fun Context.withAppLocale(): Context {
    val tags = AppCompatDelegate.getApplicationLocales().toLanguageTags()
    if (tags.isEmpty()) {
        // No choice made: the system language is the right one, and asking for
        // a configuration context would only copy it.
        return this
    }
    val configuration = Configuration(resources.configuration)
    configuration.setLocales(LocaleList.forLanguageTags(tags))
    return createConfigurationContext(configuration)
}
