package com.warrenbrowse.vpn.feature.applisting.impl

import android.content.res.Resources
import com.warrenbrowse.vpn.feature.applisting.api.AppListingTarget
import com.warrenbrowse.vpn.feature.applisting.api.ResolveAppListingUseCase
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.ui.resource.R

class ResolveAppListingUseCaseImpl(
    private val resources: Resources,
    private val packageName: PackageName,
    private val isPlayBuild: Boolean,
    private val installSourceProvider: InstallSourceProvider,
) : ResolveAppListingUseCase {
    override fun invoke(): AppListingTarget =
        if (isPlayBuild || installSourceProvider.isInstalledFromStore()) {
            AppListingTarget(
                listingUri = resources.getString(R.string.market_uri, packageName.value),
                errorMessage = resources.getString(R.string.uri_market_app_not_found),
            )
        } else {
            AppListingTarget(
                listingUri = resources.getString(R.string.download_url),
                errorMessage = resources.getString(R.string.uri_browser_app_not_found),
            )
        }
}
