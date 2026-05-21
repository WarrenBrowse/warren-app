package com.warrenbrowse.vpn.feature.filter.impl

sealed interface FilterScreenSideEffect {
    data object CloseScreen : FilterScreenSideEffect
}
