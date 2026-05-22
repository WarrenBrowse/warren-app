plugins {
    alias(libs.plugins.warren.android.library)
    alias(libs.plugins.kotlin.parcelize)
    alias(libs.plugins.protobuf.core)
    alias(libs.plugins.warren.unit.test)
}

android {
    namespace = "com.warrenbrowse.vpn.lib.repository"

    buildFeatures { buildConfig = true }
}

protobuf {
    protoc { artifact = libs.plugins.protobuf.protoc.get().toString() }
    plugins {
        create("java") { artifact = libs.plugins.grpc.protoc.gen.grpc.java.get().toString() }
    }
    generateProtoTasks {
        all().forEach {
            it.plugins { create("java") { option("lite") } }
            it.builtins { create("kotlin") { option("lite") } }
        }
    }
}

dependencies {
    implementation(projects.lib.ui.resource)
    implementation(projects.lib.common)
    // D.4 step 58: lib.grpc dropped (Mullvad daemon gRPC bridge dead).
    implementation(projects.lib.model)
    // D.4 step 36: lib.payment dropped (Mullvad billing dead on Warren).
    implementation(projects.lib.endpoint)

    implementation(libs.arrow)
    implementation(libs.arrow.optics)
    implementation(libs.arrow.resilience)
    implementation(libs.kermit)
    implementation(libs.kotlin.stdlib)
    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.androidx.datastore)
    implementation(libs.androidx.fragment)
    implementation(libs.protobuf.kotlin.lite)
}
