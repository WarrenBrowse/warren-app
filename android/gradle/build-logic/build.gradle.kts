plugins {
    `kotlin-dsl`
    alias(libs.plugins.ktfmt)
}

ktfmt {
    kotlinLangStyle()
    maxWidth.set(100)
    removeUnusedImports.set(true)
}

dependencies {
    implementation(libs.android.gradle.plugin)
    implementation(libs.kotlin.gradle.plugin)
    implementation(libs.android.gradle.junit5)
}

gradlePlugin {
    plugins {
        register("kotlin-toolchain") {
            id = "warren.kotlin-toolchain"
            implementationClass = "KotlinToolchainPlugin"
        }
    }
    plugins {
        register("utilities") {
            id = "warren.utilities"
            implementationClass = "UtilitiesPlugin"
        }
    }
    plugins {
        register("unit-test") {
            id = "warren.unit-test"
            implementationClass = "UnitTestPlugin"
        }
    }
    plugins {
        register("android-library") {
            id = "warren.android-library"
            implementationClass = "AndroidLibraryPlugin"
        }
    }
    plugins {
        register("android-library-feature-impl") {
            id = "warren.android-library-feature-impl"
            implementationClass = "AndroidLibraryFeatureImplPlugin"
        }
    }
    plugins {
        register("android-library-feature-api") {
            id = "warren.android-library-feature-api"
            implementationClass = "AndroidLibraryFeatureApiPlugin"
        }
    }
    plugins {
        register("android-library-compose") {
            id = "warren.android-library-compose"
            implementationClass = "AndroidLibraryComposePlugin"
        }
    }
    plugins {
        register("android-library-instrumented-test") {
            id = "warren.android-library-instrumented-test"
            implementationClass = "AndroidLibraryInstrumentedTestPlugin"
        }
    }
}
