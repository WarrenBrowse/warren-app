import utilities.BuildTypes
import utilities.FlavorDimensions
import utilities.Flavors

plugins {
    alias(libs.plugins.warren.utilities)
    alias(libs.plugins.android.library)
    alias(libs.plugins.kotlin.parcelize)
}

android {
    namespace = "com.warrenbrowse.vpn.test.common"
    compileSdk = libs.versions.compile.sdk.major.get().toInt()
    compileSdkMinor = libs.versions.compile.sdk.minor.get().toInt()
    buildToolsVersion = libs.versions.build.tools.get()

    defaultConfig { minSdk = libs.versions.min.sdk.get().toInt() }

    kotlin { compilerOptions { allWarningsAsErrors = true } }

    lint {
        lintConfig = file("${rootProject.projectDir}/config/lint.xml")
        abortOnError = true
        warningsAsErrors = true
    }

    packaging {
        resources {
            pickFirsts +=
                setOf(
                    // Fixes packaging error caused by: jetified-junit-*
                    "META-INF/LICENSE.md",
                    "META-INF/LICENSE-notice.md",
                )
        }
    }

    // BILLING flavor dimension dropped (Mullvad OSS/PLAY split
    // dead on Warren). Keep INFRASTRUCTURE.PROD for the baseline profile module.
    flavorDimensions += FlavorDimensions.INFRASTRUCTURE

    productFlavors { create(Flavors.PROD) { dimension = FlavorDimensions.INFRASTRUCTURE } }
}

androidComponents {
    beforeVariants { variantBuilder ->
        variantBuilder.apply { enable = name != BuildTypes.RELEASE }
    }
}

dependencies {
    implementation(projects.lib.endpoint)
    implementation(projects.lib.ui.tag)
    // lib.grpc dropped (Mullvad daemon gRPC bridge dead).
    implementation(projects.lib.model)

    implementation(libs.arrow)
    implementation(libs.androidx.test.core)
    implementation(libs.androidx.test.runner)
    implementation(libs.androidx.test.rules)
    implementation(libs.androidx.test.uiautomator)
    implementation(libs.junit.jupiter.engine)
    implementation(libs.kermit)
    implementation(libs.kotlin.stdlib)

    androidTestUtil(libs.androidx.test.orchestrator)
}
