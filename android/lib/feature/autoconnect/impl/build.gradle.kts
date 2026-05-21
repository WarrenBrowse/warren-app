plugins {
    alias(libs.plugins.mullvad.android.library)
    alias(libs.plugins.mullvad.android.library.feature.impl)
    alias(libs.plugins.mullvad.android.library.compose)
    alias(libs.plugins.kotlin.parcelize)
    alias(libs.plugins.kotlin.ksp)
}

android { namespace = "com.warrenbrowse.vpn.feature.autoconnect.impl" }

dependencies {
    implementation(projects.lib.feature.autoconnect.api)
    implementation(projects.lib.ui.util)

    implementation(libs.koin.compose)
    implementation(libs.arrow)
    implementation(libs.compose.constrainlayout)
}
