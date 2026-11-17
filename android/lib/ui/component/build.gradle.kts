plugins {
    alias(libs.plugins.warren.android.library)
    alias(libs.plugins.compose)
    alias(libs.plugins.kotlin.parcelize)
    alias(libs.plugins.warren.unit.test)
}

android {
    namespace = "com.warrenbrowse.vpn.lib.ui.component"

    buildFeatures { compose = true }
}

dependencies {
    implementation(projects.lib.common)
    implementation(projects.lib.commonCompose)
    implementation(projects.lib.model)
    implementation(projects.lib.ui.designsystem)
    implementation(projects.lib.ui.resource)
    implementation(projects.lib.ui.tag)
    implementation(projects.lib.ui.theme)
    implementation(projects.lib.ui.util)

    implementation(libs.compose.material3)
    implementation(libs.compose.ui)
    debugImplementation(libs.compose.ui.tooling)
    implementation(libs.compose.ui.tooling.preview)
    implementation(libs.compose.constrainlayout)
    implementation(libs.kermit)
    implementation(libs.kotlin.stdlib)
    implementation(libs.compose.icons.extended)
    implementation(libs.androidx.ktx)
    implementation(libs.androidx.animation)

    // Warren wallet UI (D.5) - BiometricPrompt gating in `BiometricGate.kt`.
    implementation(libs.androidx.biometric)
    implementation(libs.androidx.fragment)
}
