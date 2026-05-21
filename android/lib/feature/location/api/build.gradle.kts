plugins { alias(libs.plugins.warren.android.library.feature.api) }

android { namespace = "com.warrenbrowse.vpn.feature.location.api" }

dependencies { implementation(projects.lib.model) }
