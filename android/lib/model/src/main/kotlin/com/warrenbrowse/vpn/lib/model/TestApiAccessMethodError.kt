package com.warrenbrowse.vpn.lib.model

sealed interface TestApiAccessMethodError {
    data object CouldNotAccess : TestApiAccessMethodError

    data object Grpc : TestApiAccessMethodError

    data class Unknown(val t: Throwable) : TestApiAccessMethodError
}
