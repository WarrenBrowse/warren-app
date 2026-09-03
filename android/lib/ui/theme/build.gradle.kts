plugins {
    alias(libs.plugins.warren.android.library)
    alias(libs.plugins.compose)
    alias(libs.plugins.warren.unit.test)
}

android {
    namespace = "com.warrenbrowse.vpn.lib.ui.theme"

    buildFeatures { compose = true }
}

dependencies {
    implementation(libs.compose.material3)
    implementation(libs.compose.ui)
    implementation(libs.kotlin.stdlib)
}

// The design-token gate reads the repo-root JSON, which lives outside this
// module: declared as an input so an edit to it re-runs the gate instead of
// leaving the test up-to-date against the previous file.
tasks.withType<Test>().configureEach { inputs.file(rootProject.file("../design-tokens.json")) }
