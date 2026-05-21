plugins {
    alias(libs.plugins.warren.android.library)
    alias(libs.plugins.warren.android.library.feature.impl)
    alias(libs.plugins.warren.android.library.compose)
    alias(libs.plugins.kotlin.parcelize)
    alias(libs.plugins.kotlin.ksp)
}

android { namespace = "com.warrenbrowse.vpn.feature.serveripoverride.impl" }

dependencies {
    implementation(projects.lib.repository)
    implementation(projects.lib.usecase)
    implementation(projects.lib.navigation)

    implementation(libs.koin.compose)
    implementation(libs.arrow)
    implementation(projects.lib.feature.serveripoverride.api)
}
