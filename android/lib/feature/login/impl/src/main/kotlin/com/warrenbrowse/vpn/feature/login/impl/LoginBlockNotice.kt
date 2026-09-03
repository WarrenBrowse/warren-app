package com.warrenbrowse.vpn.feature.login.impl

import com.warrenbrowse.vpn.lib.model.TunnelState

/**
 * Why the login screen says the internet is blocked (desktop `LoginView` `BlockMessage`): lockdown
 * mode keeps the block on purpose and offers "Disable", the plain kill switch is blocking a dropped
 * tunnel and offers "Unblock".
 */
enum class LoginBlockNotice {
    LockdownMode,
    KillSwitch,
}

/**
 * The notice for the current tunnel state, or `null` while nothing is blocked. Only a blocking
 * error state counts: a tunnel that is up, dialling or released leaves the login screen alone.
 */
fun loginBlockNotice(tunnelState: TunnelState, lockdownMode: Boolean): LoginBlockNotice? {
    val blocking = tunnelState is TunnelState.Error && tunnelState.errorState.isBlocking
    return when {
        !blocking -> null
        lockdownMode -> LoginBlockNotice.LockdownMode
        else -> LoginBlockNotice.KillSwitch
    }
}
