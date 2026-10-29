plugins {
    alias(libs.plugins.warren.android.library)
    alias(libs.plugins.warren.android.library.feature.impl)
    alias(libs.plugins.warren.android.library.compose)
    alias(libs.plugins.kotlin.parcelize)
    alias(libs.plugins.kotlin.ksp)
}

android { namespace = "com.warrenbrowse.vpn.feature.managedevices.impl" }

dependencies {
    implementation(projects.lib.feature.managedevices.api)
    // "Log out" on this device ends on the wallet login, and its backup escape on the keys screen.
    implementation(projects.lib.feature.login.api)
    implementation(projects.lib.repository)
    implementation(projects.lib.usecase)
    implementation(libs.androidx.fragment)

    implementation(libs.koin.compose)
    implementation(libs.arrow)
}
