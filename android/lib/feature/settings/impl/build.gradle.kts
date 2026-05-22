plugins {
    alias(libs.plugins.warren.android.library)
    alias(libs.plugins.warren.android.library.feature.impl)
    alias(libs.plugins.warren.android.library.compose)
    alias(libs.plugins.kotlin.parcelize)
    alias(libs.plugins.kotlin.ksp)
}

android { namespace = "com.warrenbrowse.vpn.feature.settings.impl" }

dependencies {
    // D.4 step 34: anticensorship.api dep dropped.
    // D.4 step 33: apiaccess.api dep dropped (NavKey unused).
    implementation(projects.lib.feature.appearance.api)
    implementation(projects.lib.feature.appinfo.api)
    implementation(projects.lib.feature.autoconnect.api)
    // D.4 step 32: daita.api + multihop.api deps dropped (NavKeys
    // unused since onMultihopClick/onDaitaClick rewired to
    // WarrenTunnelSettings).
    implementation(projects.lib.feature.notification.api)
    implementation(projects.lib.feature.problemreport.api)
    implementation(projects.lib.feature.settings.api)
    implementation(projects.lib.feature.splittunneling.api)
    // D.4 step 53: feature.vpnsettings.api dropped (module deleted).
    implementation(projects.lib.repository)
    // D.5 wallet UI: WarrenWalletSettingsSection consumes Mnemonic /
    // WalletState (lib/model) + MnemonicDisplay / BiometricPromptAuthorizer
    // (lib/ui/component).
    implementation(projects.lib.model)
    implementation(projects.lib.ui.component)
    implementation(libs.androidx.fragment)

    implementation(libs.koin.compose)
    implementation(libs.arrow)
    implementation(libs.protobuf.kotlin.lite)
}
