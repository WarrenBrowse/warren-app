plugins {
    alias(libs.plugins.warren.android.library)
    alias(libs.plugins.kotlin.parcelize)
}

android { namespace = "com.warrenbrowse.talpid" }

dependencies {
    implementation(projects.lib.model)
    implementation(projects.lib.common)

    implementation(libs.androidx.ktx)
    implementation(libs.androidx.lifecycle.service)
    implementation(libs.arrow)
    implementation(libs.kermit)
    implementation(libs.kotlin.stdlib)
    implementation(libs.kotlinx.coroutines.android)
}
