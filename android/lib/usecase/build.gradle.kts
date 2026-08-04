plugins {
    alias(libs.plugins.warren.android.library)
    alias(libs.plugins.kotlin.parcelize)
    alias(libs.plugins.warren.unit.test)
}

android {
    namespace = "com.warrenbrowse.vpn.lib.usecase"

    buildFeatures { buildConfig = true }
}

dependencies {
    implementation(projects.lib.common)
    implementation(projects.lib.model)
    implementation(projects.lib.repository)

    implementation(libs.arrow)
    implementation(libs.arrow.optics)
    implementation(libs.kermit)
    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.androidx.annotation.jvm)
}
