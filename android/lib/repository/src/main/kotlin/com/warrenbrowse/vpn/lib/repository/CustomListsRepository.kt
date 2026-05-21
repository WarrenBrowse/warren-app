package com.warrenbrowse.vpn.lib.repository

import arrow.core.Either
import arrow.core.raise.either
import arrow.core.raise.ensureNotNull
import co.touchlab.kermit.Logger
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.mapNotNull
import kotlinx.coroutines.flow.stateIn
import com.warrenbrowse.vpn.lib.grpc.ManagementService
import com.warrenbrowse.vpn.lib.model.CustomList
import com.warrenbrowse.vpn.lib.model.CustomListId
import com.warrenbrowse.vpn.lib.model.CustomListName
import com.warrenbrowse.vpn.lib.model.GeoLocationId
import com.warrenbrowse.vpn.lib.model.GetCustomListError
import com.warrenbrowse.vpn.lib.model.UpdateCustomListLocationsError
import com.warrenbrowse.vpn.lib.model.UpdateCustomListNameError

class CustomListsRepository(
    private val managementService: ManagementService,
    dispatcher: CoroutineDispatcher = Dispatchers.IO,
) {
    val customLists: StateFlow<List<CustomList>?> =
        managementService.settings
            .mapNotNull { it.customLists.sortedByName() }
            .stateIn(CoroutineScope(dispatcher), SharingStarted.Eagerly, null)

    suspend fun createCustomList(
        name: CustomListName,
        locations: List<GeoLocationId> = emptyList(),
    ) = managementService.createCustomList(name, locations)

    suspend fun deleteCustomList(id: CustomListId) = managementService.deleteCustomList(id)

    suspend fun updateCustomList(customList: CustomList) =
        managementService.updateCustomList(customList)

    suspend fun updateCustomListName(
        id: CustomListId,
        name: CustomListName,
    ): Either<UpdateCustomListNameError, Unit> = either {
        val customList = getCustomListById(id).bind()
        updateCustomList(customList.copy(name = name))
            .mapLeft(UpdateCustomListNameError::from)
            .bind()
    }

    suspend fun updateCustomListLocations(
        id: CustomListId,
        locations: List<GeoLocationId>,
    ): Either<UpdateCustomListLocationsError, Unit> = either {
        val customList = getCustomListById(id).bind()
        updateCustomList(customList.copy(locations = locations))
            .mapLeft(UpdateCustomListLocationsError::from)
            .bind()
    }

    /**
     * There is no guarantee this will return a up to date custom list. E.g if you invoked
     * updateCustomList just before this you might get an out of date value.
     */
    fun getCustomListById(id: CustomListId): Either<GetCustomListError, CustomList> = either {
        val customLists = customLists.value
        ensureNotNull(customLists) {
            Logger.e("Custom lists never loaded")
            GetCustomListError(id)
        }
        val foundList = customLists.firstOrNull { customList -> customList.id == id }
        ensureNotNull(foundList) {
            Logger.e("Custom list with id $id not found in custom lists")
            GetCustomListError(id)
        }
    }

    private fun List<CustomList>.sortedByName() =
        this.sortedWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it.name.value })
}
