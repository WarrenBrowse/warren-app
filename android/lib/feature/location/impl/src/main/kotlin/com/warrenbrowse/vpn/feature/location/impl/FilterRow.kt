package com.warrenbrowse.vpn.feature.location.impl

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import com.warrenbrowse.vpn.lib.model.Ownership
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenFilterChip
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.usecase.FilterChip

@Preview
@Composable
private fun PreviewFilterCell() {
    AppTheme {
        Column(modifier = Modifier.background(MaterialTheme.colorScheme.surface)) {
            FilterRow(
                filters =
                    listOf(FilterChip.Ownership(Ownership.MullvadOwned), FilterChip.Provider(2)),
                onRemoveOwnershipFilter = {},
                onRemoveProviderFilter = {},
            )
        }
    }
}

@Composable
fun FilterRow(
    filters: List<FilterChip>,
    modifier: Modifier = Modifier,
    onRemoveOwnershipFilter: () -> Unit,
    onRemoveProviderFilter: () -> Unit,
) {
    val scrollState = rememberScrollState()
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = modifier.fillMaxWidth().horizontalScroll(scrollState),
        horizontalArrangement = Arrangement.spacedBy(Dimens.chipSpace),
    ) {
        Spacer(modifier = Modifier.width(Dimens.smallPadding))
        filters.forEach {
            when (it) {
                is FilterChip.Ownership ->
                    OwnershipFilterChip(it.ownership, onRemoveOwnershipFilter)
                is FilterChip.Provider -> ProviderFilterChip(it.count, onRemoveProviderFilter)
                is FilterChip.Daita -> DaitaFilterChip()
                is FilterChip.Entry -> EntryFilterChip()
                is FilterChip.Exit -> ExitFilterChip()
                is FilterChip.Quic -> QuicFilterChip()
                is FilterChip.Lwo -> LwoFilterChip()
            }
        }
        Spacer(modifier = Modifier.width(Dimens.smallPadding))
    }
}

@Composable
fun ProviderFilterChip(providers: Int, onRemoveClick: () -> Unit) {
    WarrenFilterChip(
        text = stringResource(id = R.string.number_of_providers, providers),
        onRemoveClick = onRemoveClick,
        enabled = true,
    )
}

@Composable
fun OwnershipFilterChip(ownership: Ownership, onRemoveClick: () -> Unit) {
    WarrenFilterChip(
        text = stringResource(ownership.stringResources()),
        onRemoveClick = onRemoveClick,
        enabled = true,
    )
}

@Composable
fun DaitaFilterChip() {
    WarrenFilterChip(
        text = stringResource(id = R.string.setting_chip, stringResource(id = R.string.daita)),
        onRemoveClick = {},
        enabled = false,
    )
}

@Composable
fun EntryFilterChip() {
    WarrenFilterChip(
        text = stringResource(id = R.string.entry),
        onRemoveClick = {},
        enabled = false,
    )
}

@Composable
fun ExitFilterChip() {
    WarrenFilterChip(
        text = stringResource(id = R.string.exit),
        onRemoveClick = {},
        enabled = false,
    )
}

@Composable
fun QuicFilterChip() {
    WarrenFilterChip(
        text = stringResource(id = R.string.quic),
        onRemoveClick = {},
        enabled = false,
    )
}

@Composable
fun LwoFilterChip() {
    WarrenFilterChip(text = stringResource(id = R.string.lwo), onRemoveClick = {}, enabled = false)
}

private fun Ownership.stringResources(): Int =
    when (this) {
        Ownership.MullvadOwned -> R.string.owned
        Ownership.Rented -> R.string.rented
    }
