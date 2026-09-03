package com.warrenbrowse.vpn.app

import com.warrenbrowse.vpn.BuildConfig
import com.warrenbrowse.vpn.app.forum.FORUM_HOST
import com.warrenbrowse.vpn.app.forum.forumLoginLinkFromCode
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.string
import java.net.URI
import kotlinx.serialization.json.jsonObject
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

/**
 * The Android reader of `fixtures/client-rules/product_env.json`: the flavor
 * this test binary was compiled for must carry the fixture's row, so the
 * Gradle table cannot drift from the Rust crate and the desktop tables that
 * replay the same file. Runs once per flavor under `testAllUnitTests`.
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
}
