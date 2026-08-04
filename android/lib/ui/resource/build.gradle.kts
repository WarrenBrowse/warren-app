plugins {
    alias(libs.plugins.warren.android.library)
    alias(libs.plugins.kotlin.parcelize)
}

android { namespace = "com.warrenbrowse.vpn.lib.ui.resource" }

dependencies {
    implementation(libs.androidx.appcompat)
    implementation(libs.androidx.coresplashscreen)
    implementation(libs.compose.ui)
}
