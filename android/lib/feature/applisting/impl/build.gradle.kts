plugins {
    alias(libs.plugins.mullvad.android.library)
    alias(libs.plugins.mullvad.android.library.feature.impl)
}

android { namespace = "com.warrenbrowse.vpn.feature.applisting.impl" }

dependencies { implementation(projects.lib.feature.applisting.api) }
