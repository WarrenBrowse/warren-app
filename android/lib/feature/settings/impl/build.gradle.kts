plugins {
    alias(libs.plugins.warren.android.library)
    alias(libs.plugins.warren.android.library.feature.impl)
    alias(libs.plugins.warren.android.library.compose)
    alias(libs.plugins.kotlin.parcelize)
    alias(libs.plugins.kotlin.ksp)
}

android { namespace = "com.warrenbrowse.vpn.feature.settings.impl" }

dependencies {
    // Language is surfaced directly in Settings (the only Android-applicable
    // user-interface setting).
    implementation(projects.lib.feature.language.api)
    // Wallet erase routes back to the login screen (WarrenWalletNavKey).
    implementation(projects.lib.feature.login.api)
    implementation(projects.lib.feature.appinfo.api)
    implementation(projects.lib.feature.autoconnect.api)
    implementation(projects.lib.feature.notification.api)
    implementation(projects.lib.feature.settings.api)
    implementation(projects.lib.feature.splittunneling.api)
    implementation(projects.lib.repository)
    // WarrenWalletSettingsSection consumes Mnemonic / WalletState (lib/model)
    // + MnemonicDisplay / BiometricPromptAuthorizer (lib/ui/component).
    implementation(projects.lib.model)
    implementation(projects.lib.ui.component)
    implementation(projects.lib.ui.designsystem)
    implementation(projects.lib.ui.theme)
    implementation(libs.androidx.fragment)

    implementation(libs.koin.compose)
    implementation(libs.arrow)
    implementation(libs.protobuf.kotlin.lite)
}
