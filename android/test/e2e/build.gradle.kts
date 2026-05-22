import utilities.FlavorDimensions
import utilities.Flavors
import utilities.Variant
import utilities.matches
import utilities.prodDebug

plugins {
    alias(libs.plugins.warren.utilities)
    alias(libs.plugins.android.test)
    alias(libs.plugins.kotlinx.serialization)
}

android {
    namespace = "com.warrenbrowse.vpn.test.e2e"
    compileSdk = libs.versions.compile.sdk.major.get().toInt()
    compileSdkMinor = libs.versions.compile.sdk.minor.get().toInt()
    buildToolsVersion = libs.versions.build.tools.get()

    defaultConfig {
        minSdk = libs.versions.min.sdk.get().toInt()
        testApplicationId = "com.warrenbrowse.vpn.test.e2e"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        targetProjectPath = ":app"

        testInstrumentationRunnerArguments += buildMap {
            put("clearPackageData", "true")

            // Add all properties starting with "warren.test.e2e" to the
            // testInstrumentationRunnerArguments
            properties.forEach {
                if (it.key.startsWith("warren.test.e2e")) {
                    put(it.key, it.value.toString())
                }
            }
        }
    }

    // D.4 step 64: BILLING flavor dropped.
    flavorDimensions += FlavorDimensions.INFRASTRUCTURE

    productFlavors {
        create(Flavors.PROD) {
            dimension = FlavorDimensions.INFRASTRUCTURE
            buildConfigField(
                type = "String",
                name = "INFRASTRUCTURE_BASE_DOMAIN",
                value = "\"warrenbrowse.com\"",
            )
        }
    }

    testOptions { execution = "ANDROIDX_TEST_ORCHESTRATOR" }

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
    buildFeatures { buildConfig = true }
}

androidComponents {
    beforeVariants { variantBuilder ->
        variantBuilder.enable =
            Variant(variantBuilder.buildType, variantBuilder.productFlavors)
                .matches(prodDebug)
    }
}

dependencies {
    implementation(projects.test.common)
    implementation(projects.lib.endpoint)
    implementation(projects.lib.ui.tag)
    implementation(projects.lib.model)

    implementation(libs.androidx.test.core)
    // Fixes: https://github.com/android/android-test/issues/1589
    implementation(libs.androidx.test.monitor)
    implementation(libs.androidx.test.runner)
    implementation(libs.androidx.test.rules)
    implementation(libs.androidx.test.uiautomator)
    implementation(libs.androidx.ui.test)
    implementation(libs.kermit)
    implementation(libs.junit.jupiter.api)
    implementation(libs.junit5.android.test.core)
    implementation(libs.junit5.android.test.compose)
    implementation(libs.junit5.android.test.extensions)
    implementation(libs.junit5.android.test.runner)
    implementation(libs.kotlin.stdlib)
    implementation(libs.ktor.client.core)
    implementation(libs.ktor.client.cio)
    implementation(libs.ktor.client.auth)
    implementation(libs.ktor.client.logging)
    implementation(libs.ktor.serialization.kotlinx.json)
    implementation(libs.ktor.client.content.negotiation)
    implementation(libs.ktor.client.resources)
    androidTestUtil(libs.androidx.test.orchestrator)

    // Needed or else the app crashes when launched
    implementation(libs.compose.material3)

    // Need these for forcing later versions of dependencies
    implementation(libs.compose.ui)
    implementation(libs.androidx.activity.compose)
}
