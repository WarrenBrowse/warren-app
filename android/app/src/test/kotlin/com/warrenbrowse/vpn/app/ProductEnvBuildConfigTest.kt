package com.warrenbrowse.vpn.app

import com.warrenbrowse.vpn.BuildConfig
import com.warrenbrowse.vpn.app.forum.FORUM_HOST
import com.warrenbrowse.vpn.app.forum.forumLoginLinkFromCode
import com.warrenbrowse.vpn.app.product.ProductAnchors
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.string
import java.io.File
import java.net.URI
import kotlinx.serialization.json.jsonObject
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

/**
 * The Android reader of `fixtures/client-rules/product_env.json`: the flavor
 * this test binary was compiled for must carry the fixture's row, so the
 * Gradle table cannot drift from the Rust crate and the desktop tables that
 * replay the same file.
 *
 * `testAllUnitTests` resolves to `testProdDebugUnitTest` for a flavored module
 * (`UnitTestPlugin`), so on CI this reader only ever runs for the prod flavor.
 * The beta and staging rows are held by
 * `warren-product-env/tests/platform_lockstep.rs::the_android_flavor_table_is_the_crates`,
 * which reads `android/app/build.gradle.kts` as text for every environment. Do
 * not add a reader here for a per-flavor fact that only exists at runtime: it
 * would never run for two of the three rows.
 */
class ProductEnvBuildConfigTest {
    private val row =
        ClientRulesFixtures.load("product_env.json")["environments"]!!
            .jsonObject[BuildConfig.FLAVOR]
            ?.jsonObject ?: error("product_env.json has no row for the ${BuildConfig.FLAVOR} flavor")

    @Test
    fun the_flavor_registers_the_fixtures_deep_link_scheme() {
        // The manifest placeholder and this field are set side by side in the
        // flavor block; a beta build answering the prod scheme would steal the
        // prod install's links on a device carrying both.
        assertEquals(row.string("deep_link_scheme"), BuildConfig.DEEP_LINK_SCHEME)
    }

    @Test
    fun the_flavor_installs_under_the_fixtures_application_id() {
        assertEquals(row.string("application_id"), BuildConfig.APPLICATION_ID)
    }

    @Test
    fun the_flavor_points_at_the_fixtures_api_host() {
        // The prod flavor leaves the override slot empty and runs on the host
        // the Rust crate compiles in; the others name their host explicitly.
        val expected = if (BuildConfig.FLAVOR == "prod") "" else row.string("api_host")
        assertEquals(expected, BuildConfig.API_ENDPOINT)
    }

    @Test
    fun the_forum_and_connect_hosts_are_the_fixtures() {
        assertEquals(URI(row.string("forum_public_url")).host, FORUM_HOST)
        assertEquals(row.string("connect_host"), forumLoginLinkFromCode("0".repeat(32)).host)
    }

    @Test
    fun the_update_banner_opens_the_fixtures_download_page() {
        // Topic 204: the beta app fell through to the library default and
        // opened the production download page. The flavor's overlay wins
        // over the library default, as the resource merger resolves it.
        assertEquals(row.string("download_url"), flavorString("download_url"))
    }

    /** [name] as this flavor resolves it: its own overlay, else the library default. */
    private fun flavorString(name: String): String {
        val pattern = Regex("<string name=\"$name\"[^>]*>([^<]*)</string>")
        val overlay = File("src/${BuildConfig.FLAVOR}/res/values/strings_non_translatable.xml")
        val default = File("../lib/ui/resource/src/main/res/values/strings_non_translatable.xml")
        return listOf(overlay, default)
            .filter(File::isFile)
            .firstNotNullOfOrNull { pattern.find(it.readText())?.groupValues?.get(1) }
            ?: error("no `$name` string for the ${BuildConfig.FLAVOR} flavor")
    }

    @Test
    fun the_flavor_is_the_row_the_native_table_hands_kotlin() {
        // The Rust replay pins `anchors_json()` to this fixture row, so decoding the
        // row is decoding what `WarrenJni.productAnchorsJson()` returns on a device of
        // this flavor. The JVM cannot load the native library; the instrumented
        // `ProductAnchorsJniTest` reads the real table.
        val anchors = ProductAnchors.fromJson(row.toString())
        assertEquals(BuildConfig.FLAVOR, anchors.name)
        assertEquals(BuildConfig.APPLICATION_ID, anchors.applicationId)
        assertEquals(BuildConfig.DEEP_LINK_SCHEME, anchors.deepLinkScheme)
        assertEquals(
            if (BuildConfig.FLAVOR == "prod") "" else anchors.apiHost,
            BuildConfig.API_ENDPOINT,
        )
        assertEquals(FORUM_HOST, URI(anchors.forumPublicUrl).host)
        assertEquals(anchors.connectHost, forumLoginLinkFromCode("0".repeat(32)).host)
    }
}
