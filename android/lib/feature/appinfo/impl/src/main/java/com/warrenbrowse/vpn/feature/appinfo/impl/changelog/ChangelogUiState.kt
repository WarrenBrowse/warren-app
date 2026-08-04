package com.warrenbrowse.vpn.feature.appinfo.impl.changelog

import android.os.Parcelable
import kotlinx.parcelize.Parcelize

@Parcelize
data class ChangelogUiState(
    val isModal: Boolean = false,
    val version: String,
    val changes: List<String>,
) : Parcelable
