plugins {
    alias(libs.plugins.warren.android.library)
    alias(libs.plugins.warren.android.library.feature.impl)
    alias(libs.plugins.warren.android.library.compose)
    alias(libs.plugins.kotlin.parcelize)
    alias(libs.plugins.kotlin.ksp)
}

android { namespace = "com.warrenbrowse.vpn.feature.language.impl" }

dependencies {
    implementation(projects.lib.feature.language.api)

    // AppCompat carries the per-app language below API 33, where the framework
    // LocaleManager does not exist.
    implementation(libs.androidx.appcompat)
    implementation(libs.koin.compose)
}
