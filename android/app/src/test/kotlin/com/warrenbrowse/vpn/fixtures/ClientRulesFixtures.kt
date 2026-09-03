package com.warrenbrowse.vpn.fixtures

import java.io.File
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/**
 * The cross-platform client-rule fixtures (`fixtures/client-rules/README.md` at
 * the repository root), read here the way the Rust crates and the desktop
 * unit suite read them: one file, several readers, no copy. Located by walking
 * up from the module's working directory, so the tests run from Gradle and
 * from an IDE alike.
 */
object ClientRulesFixtures {
    private const val DIR = "fixtures/client-rules"

    fun load(name: String): JsonObject =
        Json.parseToJsonElement(File(dir(), name).readText()).jsonObject

    /** Whether the Android reader must leave [case] alone (a divergence a later lot closes). */
    fun skippedOnAndroid(case: JsonObject): Boolean =
        case["skip"]?.jsonArray?.any { it.jsonPrimitive.content == "android" } == true

    fun JsonObject.string(key: String): String =
        this[key]?.jsonPrimitive?.contentOrNull ?: error("`$key` is missing in $this")

    /** A string field that the fixture may set to `null` on purpose. */
    fun JsonObject.stringOrNull(key: String): String? = this[key]?.jsonPrimitive?.contentOrNull

    fun JsonObject.cases(key: String): List<JsonObject> =
        (this[key] ?: error("`$key` is missing in $this")).jsonArray.map { it.jsonObject }

    private fun dir(): File {
        var dir: File? = File("").absoluteFile
        while (dir != null) {
            val candidate = File(dir, DIR)
            if (candidate.isDirectory) return candidate
            dir = dir.parentFile
        }
        error("could not locate $DIR from ${File("").absolutePath}")
    }
}
