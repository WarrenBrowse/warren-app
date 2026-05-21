plugins {
    alias(libs.plugins.warren.android.library)
    alias(libs.plugins.warren.android.library.feature.impl)
    alias(libs.plugins.warren.android.library.compose)
    alias(libs.plugins.kotlin.parcelize)
    alias(libs.plugins.kotlin.ksp)
}

android { namespace = "com.warrenbrowse.vpn.feature.settings.impl" }

dependencies {
    implementation(projects.lib.feature.anticensorship.api)
    implementation(projects.lib.feature.apiaccess.api)
    implementation(projects.lib.feature.appearance.api)
    implementation(projects.lib.feature.appinfo.api)
    implementation(projects.lib.feature.autoconnect.api)
    implementation(projects.lib.feature.daita.api)
    implementation(projects.lib.feature.multihop.api)
    implementation(projects.lib.feature.notification.api)
    implementation(projects.lib.feature.problemreport.api)
    implementation(projects.lib.feature.settings.api)
    implementation(projects.lib.feature.splittunneling.api)
    implementation(projects.lib.feature.vpnsettings.api)
    implementation(projects.lib.repository)

    implementation(libs.koin.compose)
    implementation(libs.arrow)
    implementation(libs.protobuf.kotlin.lite)
}
