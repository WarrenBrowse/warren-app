plugins {
    alias(libs.plugins.warren.android.library)
    alias(libs.plugins.warren.android.library.feature.impl)
    alias(libs.plugins.warren.android.library.compose)
    alias(libs.plugins.kotlin.parcelize)
    alias(libs.plugins.kotlin.ksp)
}

android { namespace = "com.warrenbrowse.vpn.feature.account.impl" }

dependencies {
    implementation(projects.lib.feature.account.api)
    implementation(projects.lib.feature.addtime.api)
    implementation(projects.lib.feature.addtime.impl)
    implementation(projects.lib.feature.deleteaccount.api)
    implementation(projects.lib.feature.login.api)
    implementation(projects.lib.feature.managedevices.api)
    implementation(projects.lib.feature.redeemvoucher.api)
    implementation(projects.lib.payment)
    implementation(projects.lib.repository)

    implementation(libs.koin.compose)
    implementation(libs.arrow)
}
