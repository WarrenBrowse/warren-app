package com.warrenbrowse.vpn.lib.repository

import java.time.ZonedDateTime
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import com.warrenbrowse.vpn.lib.grpc.ManagementService
import com.warrenbrowse.vpn.lib.model.AccountData
import com.warrenbrowse.vpn.lib.model.DeviceState

// D.4 step 49: AccountRepository slimmed to its two production consumers:
//   - `accountData` flow (read-only) for ProblemReportRepository + ReportProblemViewModel
//     "include account ID in support report" feature
//   - `logout()` for DeviceRevokedViewModel "log out" button
// Dropped (no production consumer): createAccount, login, fetchAccountHistory,
// clearAccountHistory, onVoucherRedeemed, resetIsNewAccount, isNewAccount
// StateFlow, accountHistory StateFlow, deleteAccount, refreshAccountData,
// _isNewAccount + _mutableAccountHistory + _mutableAccountDataCache. These
// were all Mullvad account-number login/voucher/delete-account/new-account flow
// touchpoints, all replaced by the BIP39 wallet identity on Warren.
class AccountRepository(
    private val managementService: ManagementService,
    @Suppress("UNUSED_PARAMETER") deviceRepository: DeviceRepository,
    val scope: CoroutineScope,
) {
    private var lastSuccessfulAccountDataFetch: ZonedDateTime? = null

    val accountData: StateFlow<AccountData?> =
        managementService.deviceState
            .map { deviceState ->
                when (deviceState) {
                    is DeviceState.LoggedIn -> {
                        managementService
                            .getAccountData(deviceState.accountNumber)
                            .getOrNull()
                            ?.also { lastSuccessfulAccountDataFetch = ZonedDateTime.now() }
                    }
                    DeviceState.LoggedOut,
                    DeviceState.Revoked -> null
                }
            }
            .distinctUntilChanged()
            .stateIn(scope = scope, SharingStarted.Eagerly, null)

    suspend fun logout() = managementService.logoutAccount()
}
