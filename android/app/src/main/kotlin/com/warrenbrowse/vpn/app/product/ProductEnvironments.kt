package com.warrenbrowse.vpn.app.product

import android.content.pm.PackageManager

/**
 * The application id of the production install, the environment that outranks
 * every other one (`warren-product-env`'s precedence, prod over staging over
 * beta).
 *
 * Spelled in Kotlin rather than read from the native anchor table: that table
 * is this build's own row, handed over by the native library, and it can only
 * ever name the environment the binary was compiled for, while the lookup needs
 * to name the OTHER install. Held to `fixtures/client-rules/product_env.json`
 * by `ProductionPackageQueryTest`, which also checks that the non-prod flavor
 * manifests declare the matching `<queries>` entry.
 *
 * Production is the only environment looked for. Staging outranks beta on the
 * desktop too, but staging never ships, so the pair only ever meets on a
 * developer's device; naming it here would also cost the banner its plain copy,
 * which says "production" rather than interpolating a product name into 24
 * locales.
 */
const val PROD_APPLICATION_ID = "com.warrenbrowse.vpn"

/**
 * Whether [applicationId] is installed on this device.
 *
 * From Android 11 an application the manifest does not declare in `<queries>`
 * is invisible, and asking for it raises the same `NameNotFoundException` as a
 * package that is genuinely absent, so the entry in the non-prod flavor
 * manifests is what makes a `false` here mean anything.
 */
fun PackageManager.isApplicationInstalled(applicationId: String): Boolean =
    try {
        getPackageInfo(applicationId, 0)
        true
    } catch (_: PackageManager.NameNotFoundException) {
        false
    }
