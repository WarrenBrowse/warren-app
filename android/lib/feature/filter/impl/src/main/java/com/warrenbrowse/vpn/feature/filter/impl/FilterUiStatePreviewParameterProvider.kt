package com.warrenbrowse.vpn.feature.filter.impl

import androidx.compose.ui.tooling.preview.PreviewParameterProvider
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.Ownership
import com.warrenbrowse.vpn.lib.model.ProviderId

private val PROVIDER_TO_OWNERSHIPS = mapOf(ProviderId("provider1") to setOf(Ownership.MullvadOwned))

class FilterUiStatePreviewParameterProvider : PreviewParameterProvider<FilterUiState> {
    override val values =
        sequenceOf(
            FilterUiState(
                providerToOwnerships = PROVIDER_TO_OWNERSHIPS,
                selectedOwnership = Constraint.Only(Ownership.MullvadOwned),
                selectedProviders = Constraint.Only(PROVIDER_TO_OWNERSHIPS.keys),
            )
        )
}
