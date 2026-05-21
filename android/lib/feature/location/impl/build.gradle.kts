plugins {
    alias(libs.plugins.warren.android.library)
    alias(libs.plugins.warren.android.library.feature.impl)
    alias(libs.plugins.warren.android.library.compose)
    alias(libs.plugins.kotlin.parcelize)
    alias(libs.plugins.kotlin.ksp)
}

android { namespace = "com.warrenbrowse.vpn.feature.location.impl" }

dependencies {
    implementation(projects.lib.ui.icon)
    implementation(projects.lib.repository)
    implementation(projects.lib.usecase)
    implementation(projects.lib.feature.customlist.api)
    implementation(projects.lib.feature.daita.api)
    implementation(projects.lib.feature.filter.api)
    implementation(projects.lib.feature.location.api)

    implementation(libs.compose.constrainlayout)
    implementation(libs.koin.compose)
    implementation(libs.arrow)
}
