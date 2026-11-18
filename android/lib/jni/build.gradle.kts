plugins {
    alias(libs.plugins.warren.android.library)
    alias(libs.plugins.warren.unit.test)
}

// The Kotlin face of the native engine: the `WarrenJni` object, its runtime
// gate and the bridge onto the repository interfaces. The `.so` itself is
// built and packaged by :app; the JNI symbols follow the package, not the
// module, so this split changes nothing on the native side.
android { namespace = "com.warrenbrowse.vpn.lib.jni" }

dependencies {
    implementation(projects.lib.repository)

    implementation(libs.kermit)
    implementation(libs.kotlin.stdlib)
    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.kotlinx.serialization.json)
}
