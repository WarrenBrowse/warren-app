package com.warrenbrowse.vpn.lib.repository

import arrow.core.Either
import arrow.core.right
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import com.warrenbrowse.vpn.lib.model.AccountData
import com.warrenbrowse.vpn.lib.model.LogoutAccountError

// D.4 step 58: AccountRepository stripped of ManagementService dependency.
// The Mullvad daemon is dead on Warren — no account number, no DeviceState,
// no getAccountData. `accountData` permanently emits null (no Mullvad account
// on Warren), `logout()` is a no-op returning success. The shim is kept so
// DeviceRevokedViewModel + ProblemReportRepository compile while the
// dead-daemon path is being phased out.
@Suppress("UNUSED_PARAMETER", "unused")
class AccountRepository(
    @Suppress("UnusedPrivateMember") managementService: Any? = null,
    @Suppress("UnusedPrivateMember") deviceRepository: DeviceRepository? = null,
    val scope: Any? = null,
) {
    val accountData: StateFlow<AccountData?> = MutableStateFlow(null)

    suspend fun logout(): Either<LogoutAccountError, Unit> = Unit.right()
}
