plugins {
    alias(libs.plugins.warren.android.library)
    alias(libs.plugins.compose)
    alias(libs.plugins.warren.unit.test)
}

android {
    namespace = "com.warrenbrowse.vpn.lib.ui.designsystem"

    buildFeatures { compose = true }
}

dependencies {
    implementation(projects.lib.model)
    implementation(projects.lib.ui.tag)
    implementation(projects.lib.ui.theme)
    implementation(projects.lib.ui.util)

    implementation(libs.compose.ui)
    implementation(libs.compose.ui.tooling)
    implementation(libs.compose.ui.tooling.preview)
    implementation(libs.compose.material3)
    implementation(libs.compose.icons.extended)
}
