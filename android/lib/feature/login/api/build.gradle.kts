plugins { alias(libs.plugins.warren.android.library.feature.api) }

android { namespace = "com.warrenbrowse.vpn.feature.login.api" }

dependencies { api(projects.lib.model) }
