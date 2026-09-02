package com.warrenbrowse.vpn.lib.ui.component.listitem

import androidx.compose.foundation.layout.Column
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha60
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextDirection

@Composable
internal fun TitleAndSubtitle(
    title: String,
    subtitle: String?,
    // Desktop ListItemItemText: labelTiny 12 at 60 % white.
    subtitleColor: Color = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha60),
    subTitleStyle: TextStyle = MaterialTheme.typography.labelMedium,
    subTitleTextDirection: TextDirection = TextDirection.Unspecified,
) {
    Column {
        Text(title)
        if (subtitle != null) {
            Text(
                text = subtitle,
                style = subTitleStyle.copy(textDirection = subTitleTextDirection),
                color = subtitleColor,
            )
        }
    }
}
