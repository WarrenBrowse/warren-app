plugins {
    alias(libs.plugins.warren.android.library)
    alias(libs.plugins.warren.android.library.feature.impl)
    alias(libs.plugins.warren.android.library.compose)
    alias(libs.plugins.kotlin.parcelize)
    alias(libs.plugins.kotlin.ksp)
}

android { namespace = "com.warrenbrowse.vpn.feature.home.impl" }

dependencies {
    // D.4 step 18: home/impl no longer depends on account/addtime/redeemvoucher
    // modules ; WelcomeScreen + OutOfTimeScreen (sole consumers) are gone.
    implementation(projects.lib.feature.anticensorship.api)
    implementation(projects.lib.feature.appinfo.api)
    implementation(projects.lib.feature.applisting.api)
    implementation(projects.lib.feature.daita.api)
    implementation(projects.lib.feature.home.api)
    implementation(projects.lib.feature.login.api)
    implementation(projects.lib.feature.multihop.api)
    implementation(projects.lib.feature.serveripoverride.api)
    implementation(projects.lib.feature.settings.api)
    implementation(projects.lib.feature.splittunneling.api)
    implementation(projects.lib.feature.vpnsettings.api)
    implementation(projects.lib.map)
    implementation(projects.lib.payment)
    implementation(projects.lib.pushNotification)
    implementation(projects.lib.repository)
    implementation(projects.lib.tv)
    implementation(projects.lib.ui.util)
    implementation(projects.lib.usecase)

    implementation(libs.androidx.animation)
    implementation(libs.androidx.navigation3.ui)
    implementation(libs.koin.compose)
    implementation(libs.arrow)
    implementation(libs.compose.constrainlayout)
    implementation(libs.androidx.credentials) {
        // This dependency adds a lot of unused permissions to the app.
        // It is not used so let's exclude it.
        // Unfortunately, this is not possible to do using libs.version.toml
        // https://github.com/gradle/gradle/issues/26367#issuecomment-2120830998
        exclude("androidx.biometric", "biometric")
    }
}
