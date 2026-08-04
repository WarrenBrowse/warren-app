plugins {
    alias(libs.plugins.warren.android.library)
    alias(libs.plugins.warren.unit.test)
    alias(libs.plugins.kotlin.parcelize)
    alias(libs.plugins.kotlin.ksp)
}

android { namespace = "com.warrenbrowse.vpn.lib.model" }

dependencies {
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.kotlin.stdlib)
    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.arrow)
    implementation(libs.arrow.optics)
    ksp(libs.arrow.optics.ksp)
}
