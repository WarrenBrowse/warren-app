package com.warrenbrowse.vpn.app

import java.io.File
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * While no upload keystore exists, the beta ships as a direct APK signed with the
 * build machine's AGP debug key (`android/docs/BuildInstructions.md`, "Release build
 * without a keystore"). Three facts carry that arrangement and none of them exists
 * at runtime in a JVM test, so they are read as text: the release build type never
 * falls back to a null signing config (an unsigned APK installs nowhere), R8 keeps
 * the two classes the runtime reaches by name, and the signer fingerprint the
 * release workflow asserts is a well-formed digest. The class name carries no "Release":
 * the root build script reads any such token on the Gradle command line as a release
 * build and runs the dirty-tree preflight, which a `--tests` filter on this class would trip.
 */
class SigningFallbackTest {
    private val repoRoot: File = locateRepoRoot()

    @Test
    fun the_release_build_type_falls_back_to_the_debug_signing_config() {
        val release = releaseBuildTypeBlock()
        assertTrue(
            release.contains("signingConfigs.getByName(\"debug\")"),
            "the release build type must sign with the AGP debug key when no keystore is configured",
        )
        assertFalse(
            Regex("""else\s*\{\s*null\s*}""").containsMatchIn(release),
            "a null signing config yields an unsigned APK that no device installs",
        )
    }

    @Test
    fun r8_keeps_the_classes_the_runtime_reaches_by_name() {
        val rules =
            File(repoRoot, "android/app/proguard-rules.pro")
                .readLines()
                .map { it.trim() }
                .filterNot { it.isEmpty() || it.startsWith("#") }
        // Anchored on the whole class name: a prefix match would accept a renamed
        // class (`WarrenJniBroken`) as if the rule still named the real one.
        assertTrue(
            rules.any {
                Regex("""^-keepclasseswithmembernames\s+class\s+com\.warrenbrowse\.vpn\.jni\.WarrenJni\s*\{""")
                    .containsMatchIn(it)
            },
            "warren-jni binds its natives by symbol name on WarrenJni; the class must keep its name",
        )
        assertTrue(
            rules.any {
                Regex("""^-keep\s+class\s+com\.warrenbrowse\.vpn\.repository\.UserPreferences\s*\{""")
                    .containsMatchIn(it)
            },
            "protobuf lite resolves the datastore message fields by name",
        )
    }

    @Test
    fun the_fallback_signer_fingerprint_is_one_sha256_digest() {
        val file = File(repoRoot, "android/fallback-signer.sha256")
        assertTrue(file.isFile, "${file.path} is missing: the release workflow asserts the signer against it")
        val digest = file.readText().trim()
        assertTrue(
            Regex("^[0-9a-f]{64}$").matches(digest),
            "${file.path} must hold one 64-char lowercase hex SHA-256, got '$digest'",
        )
    }

    private fun releaseBuildTypeBlock(): String {
        val gradle = File(repoRoot, "android/app/build.gradle.kts").readText()
        val start = gradle.indexOf("getByName(BuildTypes.RELEASE) {")
        val end = gradle.indexOf("getByName(BuildTypes.DEBUG)", startIndex = start)
        check(start >= 0 && end > start) { "build.gradle.kts has no release build type block" }
        return gradle.substring(start, end)
    }

    /** Walks up from the module directory, so the test runs from Gradle and an IDE alike. */
    private fun locateRepoRoot(): File {
        var dir: File? = File("").absoluteFile
        while (dir != null) {
            if (File(dir, "android/app/build.gradle.kts").isFile) return dir
            dir = dir.parentFile
        }
        error("could not locate the repository root from ${File("").absolutePath}")
    }
}
