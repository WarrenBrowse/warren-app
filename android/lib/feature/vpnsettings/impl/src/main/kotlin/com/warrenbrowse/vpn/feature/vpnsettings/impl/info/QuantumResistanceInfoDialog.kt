package com.warrenbrowse.vpn.feature.vpnsettings.impl.info

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.lifecycle.compose.dropUnlessResumed
import com.warrenbrowse.vpn.core.EmptyNavigator
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.lib.ui.component.dialog.InfoDialog
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme

@Preview
@Composable
private fun PreviewQuantumResistanceInfoDialog() {
    AppTheme { QuantumResistanceInfo(EmptyNavigator) }
}

@Composable
fun QuantumResistanceInfo(navigator: Navigator) {
    InfoDialog(
        message = stringResource(id = R.string.quantum_resistant_info_first_paragaph),
        additionalInfo = stringResource(id = R.string.quantum_resistant_info_second_paragaph),
        onDismiss = dropUnlessResumed { navigator.goBack() },
    )
}
