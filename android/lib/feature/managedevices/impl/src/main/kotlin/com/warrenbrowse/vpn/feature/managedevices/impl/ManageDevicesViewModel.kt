package com.warrenbrowse.vpn.feature.managedevices.impl

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import arrow.core.Either
import com.warrenbrowse.vpn.lib.common.Lce
import com.warrenbrowse.vpn.lib.common.constant.VIEW_MODEL_STOP_TIMEOUT
import com.warrenbrowse.vpn.lib.model.AccountDeviceList
import com.warrenbrowse.vpn.lib.model.DeviceRegistryError
import com.warrenbrowse.vpn.lib.model.inDisplayOrder
import com.warrenbrowse.vpn.lib.model.isValidDeviceName
import com.warrenbrowse.vpn.lib.repository.DeviceRepository
import com.warrenbrowse.vpn.lib.usecase.DeviceRevocationUseCase
import com.warrenbrowse.vpn.lib.usecase.WarrenLogoutOutcome
import java.time.LocalDate
import java.time.ZoneOffset
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.WhileSubscribed
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.onStart
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

private typealias ListState = Lce<Unit, AccountDeviceList, DeviceRegistryError>

/**
 * The account's devices (warren-core doc 115): another device is disconnected by revoking it, this
 * one is logged out the same way, and the registry keeps the names the account gives them.
 */
class ManageDevicesViewModel(
    private val devices: DeviceRepository,
    private val revocation: DeviceRevocationUseCase,
    /** The registry counts days in UTC, so "today" is the UTC day. */
    private val today: () -> LocalDate = { LocalDate.now(ZoneOffset.UTC) },
) : ViewModel() {

    private val list = MutableStateFlow<ListState>(Lce.Loading(Unit))
    private val busy = MutableStateFlow<Set<String>>(emptySet())

    private val _uiSideEffect = Channel<ManageDevicesSideEffect>()
    val uiSideEffect = _uiSideEffect.receiveAsFlow()

    val uiState: StateFlow<Lce<Unit, ManageDevicesUiState, DeviceRegistryError>> =
        combine(list, busy) { list, busy ->
                when (list) {
                    is Lce.Loading -> Lce.Loading(Unit)
                    is Lce.Error -> Lce.Error(list.error)
                    is Lce.Content ->
                        Lce.Content(
                            ManageDevicesUiState(
                                devices =
                                    list.value.devices.inDisplayOrder().map {
                                        ManageDevicesItemUiState(it, isLoading = it.id in busy)
                                    },
                                olderClientSeenOn = list.value.olderClientSeenOn,
                                today = today(),
                            )
                        )
                }
            }
            .onStart { fetchDevices() }
            .stateIn(
                viewModelScope,
                SharingStarted.WhileSubscribed(VIEW_MODEL_STOP_TIMEOUT),
                Lce.Loading(Unit),
            )

    fun fetchDevices() =
        viewModelScope.launch {
            list.value = Lce.Loading(Unit)
            reload()
        }

    /** Disconnects another device of the account, after the confirmation. */
    fun removeDevice(deviceId: String) =
        act(deviceId, ManageDevicesSideEffect.FailedToDisconnect) {
            devices.revokeDevice(deviceId)
        }

    /** Renames a device of the account; a name the registry would refuse is never sent. */
    fun renameDevice(deviceId: String, name: String) {
        if (!isValidDeviceName(name)) return
        act(deviceId, ManageDevicesSideEffect.FailedToRename) {
            devices.renameDevice(deviceId, name.trim())
        }
    }

    /** "Log out" on this device: revoked by the registry, then the wallet leaves the device. */
    fun logOutThisDevice(deviceId: String) =
        viewModelScope.launch {
            setBusy(deviceId, true)
            val loggedOut =
                revocation.logOutThisDevice(deviceId).getOrNull() == WarrenLogoutOutcome.LoggedOut
            setBusy(deviceId, false)
            if (loggedOut) {
                _uiSideEffect.send(ManageDevicesSideEffect.LoggedOut)
            } else {
                _uiSideEffect.send(ManageDevicesSideEffect.FailedToLogOut)
                reload()
            }
        }

    // `NotFound` means the device is already gone: the list is read again either way.
    private fun act(
        deviceId: String,
        failure: ManageDevicesSideEffect,
        request: suspend () -> Either<DeviceRegistryError, *>,
    ) =
        viewModelScope.launch {
            setBusy(deviceId, true)
            val result = request()
            setBusy(deviceId, false)
            result.onLeft {
                when (it) {
                    DeviceRegistryError.NotFound -> Unit
                    DeviceRegistryError.Revoked -> revocation.takeInRevocation()
                    else -> _uiSideEffect.send(failure)
                }
            }
            reload()
        }

    private suspend fun reload() {
        list.value =
            devices
                .listDevices()
                .fold(
                    {
                        // The registry says this device was revoked: act on it now rather than at
                        // the watcher's next look.
                        if (it == DeviceRegistryError.Revoked) revocation.takeInRevocation()
                        Lce.Error(it)
                    },
                    { Lce.Content(it) },
                )
    }

    private fun setBusy(deviceId: String, isBusy: Boolean) {
        busy.update { if (isBusy) it + deviceId else it - deviceId }
    }
}

sealed interface ManageDevicesSideEffect {
    data object FailedToDisconnect : ManageDevicesSideEffect

    data object FailedToRename : ManageDevicesSideEffect

    data object FailedToLogOut : ManageDevicesSideEffect

    /** This device logged out: the app goes to the wallet login. */
    data object LoggedOut : ManageDevicesSideEffect
}
