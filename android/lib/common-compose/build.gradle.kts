plugins {
    alias(libs.plugins.warren.android.library)
    alias(libs.plugins.warren.android.library.compose)
    alias(libs.plugins.kotlin.parcelize)
    alias(libs.plugins.kotlin.ksp)
    alias(libs.plugins.warren.unit.test)
}

android { namespace = "com.warrenbrowse.vpn.lib.common.compose" }

dependencies {
    implementation(projects.lib.ui.resource)
    implementation(projects.lib.model)
    implementation(projects.lib.common)
    implementation(projects.lib.navigation)
    implementation(libs.arrow)
    implementation(libs.kermit)
}
