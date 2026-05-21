package com.warrenbrowse.vpn.lib.model

sealed interface SetObfuscationOptionsError {
    data class Unknown(val throwable: Throwable) : SetObfuscationOptionsError
}
