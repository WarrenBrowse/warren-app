package com.warrenbrowse.vpn.lib.ui.component.listitem

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.tooling.preview.PreviewParameter
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.ObfuscationMode
import com.warrenbrowse.vpn.lib.model.Port
import com.warrenbrowse.vpn.lib.ui.component.DividerButton
import com.warrenbrowse.vpn.lib.ui.component.R
import com.warrenbrowse.vpn.lib.ui.component.preview.SelectObfuscationListItemPreviewParameterProvider
import com.warrenbrowse.vpn.lib.ui.designsystem.Hierarchy
import com.warrenbrowse.vpn.lib.ui.designsystem.ListItemClickArea
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.tag.BUTTON_ARROW_RIGHT_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme

@Preview
@Composable
private fun PreviewObfuscationListItem(
    @PreviewParameter(SelectObfuscationListItemPreviewParameterProvider::class)
    selectedObfuscationCellData: Triple<ObfuscationMode, Constraint<Port>, Boolean>
) {
    AppTheme {
        ObfuscationModeListItem(
            hierarchy = Hierarchy.Child1,
            obfuscationMode = selectedObfuscationCellData.first,
            port = selectedObfuscationCellData.second,
            isSelected = selectedObfuscationCellData.third,
            onSelected = {},
            onNavigate = {},
        )
    }
}

@Composable
fun ObfuscationModeListItem(
    modifier: Modifier = Modifier,
    hierarchy: Hierarchy = Hierarchy.Parent,
    position: Position = Position.Single,
    obfuscationMode: ObfuscationMode,
    port: Constraint<Port>,
    isSelected: Boolean,
    onSelected: (ObfuscationMode) -> Unit,
    onNavigate: () -> Unit,
    testTag: String? = null,
    buttonRightTestTag: String = BUTTON_ARROW_RIGHT_TEST_TAG,
) {
    SelectableListItem(
        modifier = modifier,
        hierarchy = hierarchy,
        position = position,
        isSelected = isSelected,
        title = obfuscationMode.toTitle(),
        subtitle = stringResource(id = R.string.port_x, port.toSubTitle()),
        iconContentDescription = null,
        onClick = { onSelected(obfuscationMode) },
        testTag = testTag,
        mainClickArea = ListItemClickArea.LeadingAndMain,
        trailingContent = {
            DividerButton(
                modifier = Modifier.testTag(buttonRightTestTag),
                onClick = onNavigate,
                icon = Icons.AutoMirrored.Rounded.KeyboardArrowRight,
            )
        },
    )
}

@Composable
fun ObfuscationMode.toTitle() =
    when (this) {
        ObfuscationMode.Auto -> stringResource(id = R.string.automatic)
        ObfuscationMode.Off -> stringResource(id = R.string.none)
        ObfuscationMode.Udp2Tcp -> stringResource(id = R.string.udp_over_tcp)
        ObfuscationMode.Shadowsocks -> stringResource(id = R.string.shadowsocks)
        ObfuscationMode.Quic -> stringResource(id = R.string.quic)
        ObfuscationMode.Lwo -> stringResource(id = R.string.lwo)
        ObfuscationMode.WireguardPort -> stringResource(id = R.string.wireguard_port_title)
    }

@Composable
private fun Constraint<Port>.toSubTitle() =
    when (this) {
        Constraint.Any -> stringResource(id = R.string.automatic)
        is Constraint.Only -> this.value.toString()
    }
