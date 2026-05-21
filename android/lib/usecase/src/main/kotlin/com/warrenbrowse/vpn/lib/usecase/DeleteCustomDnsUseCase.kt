package com.warrenbrowse.vpn.lib.usecase

import arrow.core.raise.either
import arrow.core.raise.ensure
import com.warrenbrowse.vpn.lib.model.SetDnsOptionsError
import com.warrenbrowse.vpn.lib.repository.SettingsRepository

class DeleteCustomDnsUseCase(private val settingsRepository: SettingsRepository) {
    suspend operator fun invoke(index: Int) =
        either<SetDnsOptionsError, Int> {
            val sizePriorToDeletion =
                settingsRepository.settingsUpdates.value
                    ?.tunnelOptions
                    ?.dnsOptions
                    ?.customOptions
                    ?.addresses
                    ?.size ?: 0
            ensure(sizePriorToDeletion > 0) {
                SetDnsOptionsError.Unknown(IllegalStateException("No custom DNS entries"))
            }
            settingsRepository.deleteCustomDns(index).bind()
            sizePriorToDeletion - 1
        }
}
