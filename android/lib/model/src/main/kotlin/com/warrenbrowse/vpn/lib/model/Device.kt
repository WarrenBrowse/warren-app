package com.warrenbrowse.vpn.lib.model

import android.os.Parcelable
import java.time.ZonedDateTime
import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.lib.model.extensions.startCase

@Parcelize
data class Device(val id: DeviceId, private val name: String, val creationDate: ZonedDateTime) :
    Parcelable {
    fun displayName(): String = name.startCase()
}
