plugins {
    alias(libs.plugins.warren.android.library)
    alias(libs.plugins.warren.android.library.compose)
}

android { namespace = "com.warrenbrowse.vpn.screen.test" }

dependencies {
    implementation(projects.lib.ui.theme)
    implementation(libs.junit5.android.test.compose)
    implementation(libs.androidx.ktx)
}
