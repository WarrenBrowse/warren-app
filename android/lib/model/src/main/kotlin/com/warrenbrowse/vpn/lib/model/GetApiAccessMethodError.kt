package com.warrenbrowse.vpn.lib.model

sealed interface GetApiAccessMethodError : UpdateApiAccessMethodError {
    data object NotFound : GetApiAccessMethodError
}
