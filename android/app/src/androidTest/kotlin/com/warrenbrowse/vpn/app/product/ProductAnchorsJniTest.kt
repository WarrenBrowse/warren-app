package com.warrenbrowse.vpn.app.product

import com.warrenbrowse.vpn.BuildConfig
import com.warrenbrowse.vpn.app.forum.FORUM_HOST
import com.warrenbrowse.vpn.app.forum.forumLoginLinkFromCode
import com.warrenbrowse.vpn.jni.WarrenJni
import java.net.URI
import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

/**
 * On a device the native library is the real one: the table `libwarren_jni.so` was compiled with
 * must be this flavor's own `BuildConfig`, otherwise the Gradle flavor and the cargo build
 * disagreed on the environment and the APK registers one environment's scheme while its datapath
 * talks to another's backend. The JVM twin in `ProductEnvBuildConfigTest` decodes the fixture row
 * the Rust replay pins the table to; this one reads the table itself.
 */
class ProductAnchorsJniTest {
    @Test
    fun the_native_table_is_the_flavors_build_config() {
        val anchors = ProductAnchors.fromJson(WarrenJni.productAnchorsJson())
        assertEquals(BuildConfig.FLAVOR, anchors.name)
        assertEquals(BuildConfig.APPLICATION_ID, anchors.applicationId)
        assertEquals(BuildConfig.DEEP_LINK_SCHEME, anchors.deepLinkScheme)
        // The prod flavor leaves the override slot empty and runs on the host the
        // native library compiles in; the others name their host explicitly.
        assertEquals(
            if (BuildConfig.FLAVOR == "prod") "" else anchors.apiHost,
            BuildConfig.API_ENDPOINT,
        )
        assertEquals(FORUM_HOST, URI(anchors.forumPublicUrl).host)
        assertEquals(anchors.connectHost, forumLoginLinkFromCode("0".repeat(32)).host)
    }
}
