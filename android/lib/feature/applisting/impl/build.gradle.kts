plugins {
    alias(libs.plugins.warren.android.library)
    alias(libs.plugins.warren.android.library.feature.impl)
}

android { namespace = "com.warrenbrowse.vpn.feature.applisting.impl" }

dependencies { implementation(projects.lib.feature.applisting.api) }
