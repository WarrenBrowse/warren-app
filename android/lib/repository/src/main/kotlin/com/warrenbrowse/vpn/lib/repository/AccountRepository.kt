package com.warrenbrowse.vpn.lib.repository

import arrow.core.Either
import arrow.core.right
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import com.warrenbrowse.vpn.lib.model.AccountData
import com.warrenbrowse.vpn.lib.model.LogoutAccountError

// Warren has no account number or device state: `accountData` always emits
// null and `logout()` returns success without doing anything. Kept so that
// DeviceRevokedViewModel compiles.
@Suppress("UNUSED_PARAMETER", "unused")
class AccountRepository(
    @Suppress("UnusedPrivateMember") managementService: Any? = null,
    @Suppress("UnusedPrivateMember") deviceRepository: DeviceRepository? = null,
    val scope: Any? = null,
) {
    val accountData: StateFlow<AccountData?> = MutableStateFlow(null)

    suspend fun logout(): Either<LogoutAccountError, Unit> = Unit.right()
}
