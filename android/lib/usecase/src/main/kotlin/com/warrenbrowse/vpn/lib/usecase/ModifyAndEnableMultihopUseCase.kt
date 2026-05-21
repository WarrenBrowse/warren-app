package com.warrenbrowse.vpn.lib.usecase

import arrow.core.Either
import arrow.core.raise.either
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.repository.CustomListsRepository
import com.warrenbrowse.vpn.lib.repository.RelayListRepository
import com.warrenbrowse.vpn.lib.repository.SettingsRepository
import com.warrenbrowse.vpn.lib.repository.WireguardConstraintsRepository

class ModifyAndEnableMultihopUseCase(
    private val relayListRepository: RelayListRepository,
    private val settingsRepository: SettingsRepository,
    private val customListsRepository: CustomListsRepository,
    private val wireguardConstraintsRepository: WireguardConstraintsRepository,
) {
    suspend operator fun invoke(
        enableMultihop: Boolean,
        change: MultihopChange,
    ): Either<ModifyMultihopError, Unit> = either {
        validate(
                change = change,
                settingsRepository = settingsRepository,
                customListsRepository = customListsRepository,
            )
            .bind()
        when (change) {
                is MultihopChange.Entry ->
                    wireguardConstraintsRepository.setMultihopAndEntryLocation(
                        enableMultihop,
                        change.item.id,
                    )
                is MultihopChange.Exit ->
                    relayListRepository.updateExitRelayLocationMultihop(
                        enableMultihop,
                        change.item.id,
                    )
            }
            .mapLeft {
                Logger.e("Failed to update multihop: $it")
                ModifyMultihopError.GenericError
            }
            .bind()
    }
}
