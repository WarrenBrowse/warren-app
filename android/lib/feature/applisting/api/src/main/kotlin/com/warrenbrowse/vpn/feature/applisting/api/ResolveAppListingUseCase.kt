package com.warrenbrowse.vpn.feature.applisting.api

fun interface ResolveAppListingUseCase {
    operator fun invoke(): AppListingTarget
}
