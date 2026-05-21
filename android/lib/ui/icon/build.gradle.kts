plugins {
    alias(libs.plugins.mullvad.android.library)
    alias(libs.plugins.kotlin.parcelize)
}

android { namespace = "com.warrenbrowse.vpn.lib.ui.icon" }

dependencies { implementation(libs.compose.ui) }
