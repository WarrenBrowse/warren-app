package com.warrenbrowse.vpn.feature.home.impl.connect.connectioninfo

import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.LocalWarrenSurfaces

@Composable
fun ConnectionInfoHeader(text: String, modifier: Modifier = Modifier) {
    Text(
        modifier = modifier.padding(top = Dimens.smallPadding),
        text = text,
        style = MaterialTheme.typography.labelLarge,
        color = LocalWarrenSurfaces.current.textMuted,
        overflow = TextOverflow.Ellipsis,
    )
}
