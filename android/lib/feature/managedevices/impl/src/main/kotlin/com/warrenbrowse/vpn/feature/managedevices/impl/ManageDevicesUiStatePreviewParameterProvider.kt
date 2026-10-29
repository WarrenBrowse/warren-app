package com.warrenbrowse.vpn.feature.managedevices.impl

import androidx.compose.ui.tooling.preview.PreviewParameterProvider
import com.warrenbrowse.vpn.lib.common.Lce
import com.warrenbrowse.vpn.lib.model.AccountDevice
import com.warrenbrowse.vpn.lib.model.DeviceRegistryError
import java.time.LocalDate

class ManageDevicesUiStatePreviewParameterProvider :
    PreviewParameterProvider<Lce<Unit, ManageDevicesUiState, DeviceRegistryError>> {
    override val values =
        sequenceOf(
            Lce.Content(
                ManageDevicesUiState(
                    devices =
                        listOf(
                            device("a", "brave otter", "android", TODAY, thisDevice = true),
                            device("b", "quiet heron", "windows", TODAY.minusDays(3)),
                            device("c", "swift lynx", "cli", TODAY.minusDays(12)),
                        ),
                    olderClientSeenOn = TODAY.minusDays(1),
                    today = TODAY,
                )
            ),
            Lce.Loading(Unit),
            Lce.Error(DeviceRegistryError.Unavailable),
        )

    private fun device(
        id: String,
        name: String,
        platform: String,
        lastSeenOn: LocalDate,
        thisDevice: Boolean = false,
    ) =
        ManageDevicesItemUiState(
            AccountDevice(id, name, platform, "1.1.40", lastSeenOn, thisDevice),
            isLoading = false,
        )

    private companion object {
        val TODAY: LocalDate = LocalDate.of(2026, 10, 29)
    }
}
