package com.warrenbrowse.vpn.lib.model.extensions

fun String.startCase() =
    split(" ").joinToString(" ") { word ->
        word.replaceFirstChar { firstChar -> firstChar.uppercase() }
    }
