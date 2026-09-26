package com.warrenbrowse.vpn.lib.ui.designsystem.networkstats

import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/** The three ring sizes every surface draws. */
enum class LoadRingSize(val diameter: Dp, val stroke: Dp, val bandStroke: Dp) {
    /** A text line of the connection card: must fit inside it. */
    TINY(14.dp, 2.5.dp, 1.5.dp),

    /** Location list rows. */
    SMALL(22.dp, 3.dp, 2.dp),

    /** Exit cards and the fleet header. */
    LARGE(104.dp, 8.dp, 3.dp),
}
