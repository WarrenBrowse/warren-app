package com.warrenbrowse.vpn.lib.model

data class ErrorState(val cause: ErrorStateCause, val isBlocking: Boolean) {
    /**
     * Whether another device of the account can be what stops this one (the device limit, or every
     * session token held elsewhere): the banner then opens the account's device list (warren-core
     * doc 115).
     */
    val isCausedByAnotherDevice: Boolean
        get() =
            cause is ErrorStateCause.AuthFailed &&
                (cause.error == AuthFailedError.TooManyConnections ||
                    cause.error == AuthFailedError.NoSessionToken)
}
