package utilities

object BuildTypes {
    const val DEBUG = "debug"
    const val RELEASE = "release"

    const val NON_MINIFIED = "nonMinified"

    const val BENCHMARK = "benchmark"
}

object SigningConfigs {
    const val RELEASE = "release"
}

object FlavorDimensions {
    const val INFRASTRUCTURE = "infrastructure"
}

object Flavors {
    const val PROD = "prod"
}

data class Variant(val buildType: String?, val productFlavors: Map<String, String>) {
    constructor(
        buildType: String?,
        productFlavors: List<Pair<String, String>>,
    ) : this(buildType, productFlavors.toMap())
}

data class VariantFilter(
    val infrastructurePredicate: (infrastructure: String?) -> Boolean = { true },
    val buildTypePredicate: (buildType: String?) -> Boolean = { true },
)

fun Variant.matches(filter: VariantFilter): Boolean =
    with(filter) {
        val flavors = productFlavors.toMap()
        buildTypePredicate(buildType) &&
            infrastructurePredicate(flavors[FlavorDimensions.INFRASTRUCTURE])
    }

fun Variant.matchesAny(vararg filters: VariantFilter): Boolean = filters.any { matches(it) }

fun fullReleaseTasks(@Suppress("UNUSED_PARAMETER") appVersion: AppVersion) =
    buildList<String> {
        add("createProdReleaseDistApk")
        add("createProdReleaseDistBundle")
    }
