package com.warrenbrowse.vpn.lib.usecase.customlists

import arrow.core.Either
import arrow.core.raise.either
import arrow.core.raise.ensure
import kotlinx.coroutines.flow.firstOrNull
import com.warrenbrowse.vpn.lib.common.util.relaylist.getRelayItemsByCodes
import com.warrenbrowse.vpn.lib.model.CreateCustomListError
import com.warrenbrowse.vpn.lib.model.DeleteCustomListError
import com.warrenbrowse.vpn.lib.model.GetCustomListError
import com.warrenbrowse.vpn.lib.model.NameIsEmpty
import com.warrenbrowse.vpn.lib.model.UpdateCustomListLocationsError
import com.warrenbrowse.vpn.lib.model.UpdateCustomListNameError
import com.warrenbrowse.vpn.lib.model.communication.Created
import com.warrenbrowse.vpn.lib.model.communication.CustomListAction
import com.warrenbrowse.vpn.lib.model.communication.CustomListSuccess
import com.warrenbrowse.vpn.lib.model.communication.Deleted
import com.warrenbrowse.vpn.lib.model.communication.LocationsChanged
import com.warrenbrowse.vpn.lib.model.communication.Renamed
import com.warrenbrowse.vpn.lib.repository.CustomListsRepository
import com.warrenbrowse.vpn.lib.repository.RelayListRepository

class CustomListActionUseCase(
    private val customListsRepository: CustomListsRepository,
    private val relayListRepository: RelayListRepository,
) {
    suspend operator fun invoke(
        action: CustomListAction
    ): Either<CustomListActionError, CustomListSuccess> {
        return when (action) {
            is CustomListAction.Create -> {
                invoke(action)
            }
            is CustomListAction.Rename -> {
                invoke(action)
            }
            is CustomListAction.Delete -> {
                invoke(action)
            }
            is CustomListAction.UpdateLocations -> {
                invoke(action)
            }
        }
    }

    suspend operator fun invoke(action: CustomListAction.Rename): Either<RenameError, Renamed> =
        either {
            ensure(action.newName.value.isNotBlank()) { RenameError(NameIsEmpty(action.name)) }

            customListsRepository
                .updateCustomListName(action.id, action.newName)
                .map { Renamed(undo = action.not()) }
                .mapLeft(::RenameError)
                .bind()
        }

    suspend operator fun invoke(
        action: CustomListAction.Create
    ): Either<CreateWithLocationsError, Created> = either {
        ensure(action.name.value.isNotBlank()) {
            CreateWithLocationsError.Create(NameIsEmpty(action.name))
        }

        val customListId =
            customListsRepository
                .createCustomList(action.name, action.locations)
                .mapLeft(CreateWithLocationsError::Create)
                .bind()

        val locationNames =
            if (action.locations.isNotEmpty()) {
                relayListRepository.relayList
                    .firstOrNull()
                    ?.getRelayItemsByCodes(action.locations)
                    ?.map { it.name } ?: raise(CreateWithLocationsError.UnableToFetchRelayList)
            } else {
                emptyList()
            }

        Created(
            id = customListId,
            name = action.name,
            locationNames = locationNames,
            undo = action.not(customListId),
        )
    }

    suspend operator fun invoke(
        action: CustomListAction.Delete
    ): Either<DeleteWithUndoError, Deleted> = either {
        val customList =
            customListsRepository
                .getCustomListById(action.id)
                .mapLeft(DeleteWithUndoError::Fetch)
                .bind()
        customListsRepository
            .deleteCustomList(action.id)
            .mapLeft(DeleteWithUndoError::Delete)
            .bind()
        Deleted(undo = action.not(locations = customList.locations, name = customList.name))
    }

    suspend operator fun invoke(
        action: CustomListAction.UpdateLocations
    ): Either<UpdateLocationsError, LocationsChanged> = either {
        val customList =
            customListsRepository
                .getCustomListById(action.id)
                .mapLeft(UpdateLocationsError::Fetch)
                .bind()
        customListsRepository
            .updateCustomListLocations(action.id, action.locations)
            .mapLeft(UpdateLocationsError::UpdateLocations)
            .bind()
        LocationsChanged(
            id = action.id,
            name = customList.name,
            locations = action.locations,
            oldLocations = customList.locations,
        )
    }
}

sealed interface CustomListActionError

sealed interface CreateWithLocationsError : CustomListActionError {

    data class Create(val error: CreateCustomListError) : CreateWithLocationsError

    data object UnableToFetchRelayList : CreateWithLocationsError
}

sealed interface DeleteWithUndoError : CustomListActionError {
    data class Fetch(val error: GetCustomListError) : DeleteWithUndoError

    data class Delete(val error: DeleteCustomListError) : DeleteWithUndoError
}

data class RenameError(val error: UpdateCustomListNameError) : CustomListActionError

sealed interface UpdateLocationsError : CustomListActionError {

    data class Fetch(val error: GetCustomListError) : UpdateLocationsError

    data class UpdateLocations(val error: UpdateCustomListLocationsError) : UpdateLocationsError
}
