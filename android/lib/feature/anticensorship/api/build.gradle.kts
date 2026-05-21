plugins { alias(libs.plugins.mullvad.android.library.feature.api) }

android { namespace = "com.warrenbrowse.vpn.feature.anticensorship.api" }

dependencies { implementation(projects.lib.model) }
