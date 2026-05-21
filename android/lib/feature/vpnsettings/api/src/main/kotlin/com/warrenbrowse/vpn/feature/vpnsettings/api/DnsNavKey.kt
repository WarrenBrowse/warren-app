package com.warrenbrowse.vpn.feature.vpnsettings.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.NavResult

@Parcelize data class DnsNavKey(val index: Int? = null, val initialValue: String? = null) : NavKey2

sealed interface DnsNavResult : NavResult {
    @Parcelize data class Success(val isDnsListEmpty: Boolean) : DnsNavResult

    @Parcelize data object Error : DnsNavResult
}
