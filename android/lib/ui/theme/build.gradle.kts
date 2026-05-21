plugins {
    alias(libs.plugins.mullvad.android.library)
    alias(libs.plugins.compose)
}

android {
    namespace = "com.warrenbrowse.vpn.lib.ui.theme"

    buildFeatures { compose = true }
}

dependencies {
    implementation(libs.compose.material3)
    implementation(libs.compose.ui)
    implementation(libs.kotlin.stdlib)
}
