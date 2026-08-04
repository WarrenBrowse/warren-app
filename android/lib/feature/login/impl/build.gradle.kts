plugins {
    alias(libs.plugins.warren.android.library)
    alias(libs.plugins.warren.android.library.feature.impl)
    alias(libs.plugins.warren.android.library.compose)
    alias(libs.plugins.kotlin.parcelize)
    alias(libs.plugins.kotlin.ksp)
}

android { namespace = "com.warrenbrowse.vpn.feature.login.impl" }

dependencies {
    implementation(projects.lib.feature.home.api)
    implementation(projects.lib.feature.login.api)
    implementation(projects.lib.feature.settings.api)
    implementation(projects.lib.pushNotification)
    implementation(projects.lib.repository)
    implementation(projects.lib.usecase)
    // D.5 wallet UI: WarrenWalletLoginScreen consumes MnemonicInput from
    // lib/ui/component and WalletState from lib/model.
    implementation(projects.lib.model)
    implementation(projects.lib.ui.component)
    implementation(projects.lib.ui.designsystem)
    implementation(projects.lib.ui.theme)

    implementation(libs.koin.compose)
    implementation(libs.arrow)
}
