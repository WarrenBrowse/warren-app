package com.warrenbrowse.vpn.lib.repository

class SplashCompleteRepository {
    private var splashComplete = false

    fun isSplashComplete() = splashComplete

    fun onSplashCompleted() {
        splashComplete = true
    }
}
