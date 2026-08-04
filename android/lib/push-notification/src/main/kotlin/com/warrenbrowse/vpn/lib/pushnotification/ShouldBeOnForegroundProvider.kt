package com.warrenbrowse.vpn.lib.pushnotification

import kotlinx.coroutines.flow.Flow

interface ShouldBeOnForegroundProvider {
    val shouldBeOnForeground: Flow<Boolean>
}
