package com.warrenbrowse.vpn.lib.repository

import arrow.core.Either
import java.time.ZonedDateTime
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.merge
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import com.warrenbrowse.vpn.lib.grpc.ManagementService
import com.warrenbrowse.vpn.lib.model.AccountData
import com.warrenbrowse.vpn.lib.model.AccountNumber
import com.warrenbrowse.vpn.lib.model.ClearAccountHistoryError
import com.warrenbrowse.vpn.lib.model.CreateAccountError
import com.warrenbrowse.vpn.lib.model.DeleteAccountError
import com.warrenbrowse.vpn.lib.model.DeviceState
import com.warrenbrowse.vpn.lib.model.LoginAccountError

class AccountRepository(
    private val managementService: ManagementService,
    private val deviceRepository: DeviceRepository,
    val scope: CoroutineScope,
) {
    private var lastSuccessfulAccountDataFetch: ZonedDateTime? = null

    private val _mutableAccountDataCache: MutableSharedFlow<AccountData> = MutableSharedFlow()

    private val _isNewAccount: MutableStateFlow<Boolean> = MutableStateFlow(false)

    private val _mutableAccountHistory: MutableStateFlow<AccountNumber?> = MutableStateFlow(null)

    val isNewAccount: StateFlow<Boolean> = _isNewAccount

    val accountHistory: StateFlow<AccountNumber?> = _mutableAccountHistory

    val accountData: StateFlow<AccountData?> =
        merge(
                managementService.deviceState.map { deviceState ->
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
                },
                _mutableAccountDataCache,
            )
            .distinctUntilChanged()
            .stateIn(scope = scope, SharingStarted.Eagerly, null)

    suspend fun createAccount(): Either<CreateAccountError, AccountNumber> =
        managementService.createAccount().onRight { _isNewAccount.update { true } }

    suspend fun login(accountNumber: AccountNumber): Either<LoginAccountError, Unit> =
        managementService.loginAccount(accountNumber)

    suspend fun logout() =
        managementService.logoutAccount().onRight { _isNewAccount.update { false } }

    suspend fun fetchAccountHistory(): AccountNumber? =
        managementService
            .getAccountHistory()
            .onRight { _mutableAccountHistory.value = it }
            .getOrNull()

    suspend fun clearAccountHistory(): Either<ClearAccountHistoryError, Unit> =
        managementService.clearAccountHistory().onRight { _mutableAccountHistory.value = null }

    /*
     * Fetches the account data from the server, and updates the cache.
     * Unless force is true, it will only fetch if no fetch was made in the last minute.
     */
    suspend fun refreshAccountData(
        ignoreTimeout: Boolean = true,
        waitForDeviceState: Boolean = false,
    ) {
        // Only refresh if logged in
        val deviceState =
            if (waitForDeviceState) {
                deviceRepository.deviceState.filterNotNull().first() as? DeviceState.LoggedIn
            } else {
                deviceRepository.deviceState.value as? DeviceState.LoggedIn
            } ?: return

        if (ignoreTimeout || lastSuccessfulAccountDataFetch.canFetchAccountData()) {
            val accountData =
                managementService.getAccountData(deviceState.accountNumber).getOrNull()
            lastSuccessfulAccountDataFetch = ZonedDateTime.now()

            // Update stateflow cache, only update if device state is still logged in and using the
            // same account number
            deviceRepository.deviceState.value?.let {
                if (it is DeviceState.LoggedIn && it.accountNumber == accountData?.accountNumber) {
                    _mutableAccountDataCache.emit(accountData)
                }
            }
        }
    }

    // D.4 step 46: getWebsiteAuthToken removed (no consumer — Mullvad
    // mullvad.net/account web-account flow dead on Warren).

    internal suspend fun onVoucherRedeemed(newExpiry: ZonedDateTime) {
        accountData.value?.copy(expiryDate = newExpiry)?.let { _mutableAccountDataCache.emit(it) }
    }

    fun resetIsNewAccount() {
        _isNewAccount.value = false
    }

    suspend fun deleteAccount(): Either<DeleteAccountError, Unit> =
        managementService.deleteAccount()

    private fun ZonedDateTime?.canFetchAccountData(): Boolean =
        this == null || this.isBefore(ZonedDateTime.now().minusMinutes(1))
}
