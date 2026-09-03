plugins {
    alias(libs.plugins.warren.android.library)
    alias(libs.plugins.kotlin.parcelize)
    alias(libs.plugins.warren.unit.test)
}

android { namespace = "com.warrenbrowse.vpn.feature.pushnotifications" }

dependencies {
    implementation(projects.lib.common)
    implementation(projects.lib.model)
    implementation(projects.lib.repository)
    implementation(projects.lib.ui.resource)

    // The per-app language below API 33, where the framework LocaleManager
    // does not exist and a notification would otherwise keep the system one.
    implementation(libs.androidx.appcompat)
    implementation(libs.androidx.ktx)
    implementation(libs.androidx.lifecycle.service)
    implementation(libs.androidx.work.runtime.ktx)
    implementation(libs.arrow)
    implementation(libs.kermit)
    implementation(libs.koin.android)
    implementation(libs.protobuf.kotlin.lite)
}
