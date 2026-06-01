package utilities

import utilities.BuildTypes.BENCHMARK
import utilities.BuildTypes.DEBUG
import utilities.BuildTypes.NON_MINIFIED
import utilities.BuildTypes.RELEASE
import utilities.Flavors.PROD

// Filters constrain only on INFRASTRUCTURE (PROD) + build type.
val prodDebugReleaseVariants =
    VariantFilter(
        infrastructurePredicate = { it == PROD },
        buildTypePredicate = { buildType: String? ->
            when (buildType) {
                DEBUG,
                RELEASE -> true
                else -> false
            }
        },
    )

val baselineFilter =
    VariantFilter(
        infrastructurePredicate = { it == PROD },
        buildTypePredicate = {
            if (it == null) return@VariantFilter false
            val isBaselineBuildType =
                it.contains(NON_MINIFIED, true) || it.contains(BENCHMARK, true)
            isBaselineBuildType && it.contains(RELEASE, true)
        },
    )

val prodDebug =
    VariantFilter(
        infrastructurePredicate = { it == PROD },
        buildTypePredicate = { it == DEBUG },
    )
