package utilities

import utilities.BuildTypes.BENCHMARK
import utilities.BuildTypes.DEBUG
import utilities.BuildTypes.NON_MINIFIED
import utilities.BuildTypes.RELEASE
import utilities.Flavors.OSS
import utilities.Flavors.PLAY
import utilities.Flavors.PROD

val ossProdAnyBuildType =
    VariantFilter(
        billingPredicate = { it == OSS },
        infrastructurePredicate = { it == PROD },
        buildTypePredicate = {
            when (it) {
                DEBUG,
                RELEASE -> true
                else -> false
            }
        },
    )

val allPlayDebugReleaseVariants =
    VariantFilter(
        billingPredicate = { it == PLAY },
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
        billingPredicate = { it == PLAY },
        infrastructurePredicate = { it == PROD },
        buildTypePredicate = {
            if (it == null) return@VariantFilter false

            val isBaselineBuildType =
                it.contains(NON_MINIFIED, true) || it.contains(BENCHMARK, true)

            isBaselineBuildType && it.contains(RELEASE, true)
        },
    )

val ossProdDebug =
    VariantFilter(
        billingPredicate = { it == OSS },
        infrastructurePredicate = { it == PROD },
        buildTypePredicate = { it == DEBUG },
    )

val playProdDebug =
    VariantFilter(
        billingPredicate = { it == PLAY },
        infrastructurePredicate = { it == PROD },
        buildTypePredicate = { it == DEBUG },
    )
