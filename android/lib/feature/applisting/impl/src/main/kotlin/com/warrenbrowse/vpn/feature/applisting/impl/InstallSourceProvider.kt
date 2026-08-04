package com.warrenbrowse.vpn.feature.applisting.impl

fun interface InstallSourceProvider {
    fun isInstalledFromStore(): Boolean
}
