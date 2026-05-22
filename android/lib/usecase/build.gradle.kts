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
    // D.4 step 58: lib.grpc dropped (Mullvad daemon gRPC bridge dead).
    implementation(projects.lib.model)
    implementation(projects.lib.repository)

    implementation(libs.arrow)
    implementation(libs.arrow.optics)
    implementation(libs.kermit)
    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.androidx.annotation.jvm)
}
