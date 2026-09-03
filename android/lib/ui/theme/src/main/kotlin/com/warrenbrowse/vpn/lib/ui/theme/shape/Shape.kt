package com.warrenbrowse.vpn.lib.ui.theme.shape

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Shapes
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Shape
import com.warrenbrowse.vpn.lib.ui.theme.Dimens

val Shapes.chipShape: Shape
    @Composable
    get() {
        return RoundedCornerShape(Dimens.chipCornerRadius)
    }
