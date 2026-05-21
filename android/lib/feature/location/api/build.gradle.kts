plugins { alias(libs.plugins.mullvad.android.library.feature.api) }

android { namespace = "com.warrenbrowse.vpn.feature.location.api" }

dependencies { implementation(projects.lib.model) }
