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

    // Separate installable beta product: own applicationId suffix, own
    // launcher label, and a Rust datapath compiled with
    // WARREN_PRODUCT_ENV=beta (beta API host + beta update channel).
    const val BETA = "beta"

    // Same separation against the staging backend, so the three products can
    // sit on one device without sharing an applicationId, a launcher label or
    // a deep-link scheme.
    const val STAGING = "staging"
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
