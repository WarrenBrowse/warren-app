package com.warrenbrowse.vpn.app.product

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/**
 * The per-environment product anchors compiled into the native library (the Rust
 * `warren-product-env` crate), as `WarrenJni.productAnchorsJson()` hands them over: one object
 * whose keys are the columns of `fixtures/client-rules/product_env.json`. The Gradle flavor spells
 * the scheme and the application id again because the manifest needs them before any native code
 * runs; this table is the reference those `BuildConfig` copies are held to.
 */
data class ProductAnchors(
    val name: String,
    val apiUrl: String,
    val apiHost: String,
    val desktopUpdateUrl: String,
    val displayName: String,
    val unixProductDir: String,
    val applicationId: String,
    val deepLinkScheme: String,
    val connectHost: String,
    val forumPublicUrl: String,
) {
    companion object {
        /**
         * Decodes the native table. A missing column is a build defect (a library and a decoder
         * from two different revisions), so it throws rather than defaulting.
         */
        fun fromJson(json: String): ProductAnchors {
            val table = Json.parseToJsonElement(json).jsonObject
            fun column(key: String): String =
                table[key]?.jsonPrimitive?.contentOrNull
                    ?: error("the product anchors carry no `$key` column")
            return ProductAnchors(
                name = column("name"),
                apiUrl = column("api_url"),
                apiHost = column("api_host"),
                desktopUpdateUrl = column("desktop_update_url"),
                displayName = column("display_name"),
                unixProductDir = column("unix_product_dir"),
                applicationId = column("application_id"),
                deepLinkScheme = column("deep_link_scheme"),
                connectHost = column("connect_host"),
                forumPublicUrl = column("forum_public_url"),
            )
        }
    }
}
