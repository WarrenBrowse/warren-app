package com.warrenbrowse.vpn.lib.model.communication

import android.os.Parcelable
import kotlinx.parcelize.IgnoredOnParcel
import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.lib.model.CustomListId
import com.warrenbrowse.vpn.lib.model.CustomListName
import com.warrenbrowse.vpn.lib.model.GeoLocationId

sealed interface CustomListSuccess : Parcelable {
    val undo: CustomListAction
}

@Parcelize
data class Created(
    val id: CustomListId,
    val name: CustomListName,
    val locationNames: List<String>,
    override val undo: CustomListAction.Delete,
) : CustomListSuccess

@Parcelize
data class Deleted(override val undo: CustomListAction.Create) : CustomListSuccess {
    val name: CustomListName
        get() = undo.name
}

@Parcelize
data class Renamed(override val undo: CustomListAction.Rename) : CustomListSuccess {
    val name: CustomListName
        get() = undo.name
}

@Parcelize
data class LocationsChanged(
    val id: CustomListId,
    val name: CustomListName,
    val locations: List<GeoLocationId>,
    val oldLocations: List<GeoLocationId>,
) : CustomListSuccess {
    override val undo: CustomListAction.UpdateLocations
        get() = CustomListAction.UpdateLocations(id, oldLocations)

    @IgnoredOnParcel val addedLocations = locations - oldLocations
    @IgnoredOnParcel val removedLocations = oldLocations - locations
}
