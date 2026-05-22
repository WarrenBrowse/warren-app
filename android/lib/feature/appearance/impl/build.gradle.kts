plugins {
    alias(libs.plugins.warren.android.library)
    alias(libs.plugins.warren.android.library.feature.impl)
    alias(libs.plugins.warren.android.library.compose)
    alias(libs.plugins.kotlin.parcelize)
    alias(libs.plugins.kotlin.ksp)
}

android { namespace = "com.warrenbrowse.vpn.feature.appearance.impl" }

dependencies {
    // D.4 step 61: feature.appicon.api dropped.
    implementation(projects.lib.feature.appearance.api)
    implementation(projects.lib.feature.language.api)
    implementation(projects.lib.repository)

    implementation(libs.koin.compose)
    implementation(libs.arrow)
}
