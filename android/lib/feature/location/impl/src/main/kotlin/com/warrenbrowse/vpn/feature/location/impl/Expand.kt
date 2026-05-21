package com.warrenbrowse.vpn.feature.location.impl

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.update
import com.warrenbrowse.vpn.feature.location.impl.search.expandKey
import com.warrenbrowse.vpn.lib.model.CustomListId
import com.warrenbrowse.vpn.lib.model.RelayItemId

internal fun MutableStateFlow<Set<String>>.onToggleExpandSet(
    item: RelayItemId,
    parent: CustomListId? = null,
    expand: Boolean,
) {
    update {
        val key = item.expandKey(parent)
        if (expand) {
            it + key
        } else {
            it - key
        }
    }
}

internal fun MutableStateFlow<Map<String, Boolean>>.onToggleExpandMap(
    item: RelayItemId,
    parent: CustomListId? = null,
    expand: Boolean,
) {
    update {
        val key = item.expandKey(parent)
        it + (key to expand)
    }
}
