package com.warrenbrowse.vpn.feature.managedevices.impl.confirmation

import androidx.compose.ui.tooling.preview.PreviewParameterProvider
import java.time.ZonedDateTime
import java.time.format.DateTimeFormatter
import com.warrenbrowse.vpn.lib.model.Device
import com.warrenbrowse.vpn.lib.model.DeviceId

class ManageDeviceRemoveConfirmationPreviewParameterProvider : PreviewParameterProvider<Device> {
    override val values: Sequence<Device> =
        sequenceOf(
            Device(
                id = DeviceId.fromString("12345678-1234-5678-1234-567812345678"),
                name = "Secure Mole",
                creationDate = DEVICE_CREATION_DATE,
            )
        )

    companion object {
        private val DEVICE_CREATION_DATE =
            ZonedDateTime.parse("2024-05-27T00:00+00:00", DateTimeFormatter.ISO_ZONED_DATE_TIME)
    }
}
