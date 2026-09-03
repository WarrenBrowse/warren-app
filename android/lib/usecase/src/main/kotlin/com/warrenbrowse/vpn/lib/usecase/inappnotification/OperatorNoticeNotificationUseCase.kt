package com.warrenbrowse.vpn.lib.usecase.inappnotification

import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.repository.WarrenNoticeState
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map

/**
 * The operator broadcast banner (desktop `WarrenNoticeNotificationProvider`).
 *
 * The single banner slot shows one notice at a time, and it is the first the
 * operator published: the set arrives in publication order, and reordering it
 * here would make which message a user sees depend on a rule nobody publishing
 * one can see.
 *
 * Nothing is filtered on this side. Whether a notice may be displayed at all
 * was settled in Rust against the pinned server key, so an empty list means
 * exactly "there is nothing to show", whether it was erased, lapsed, or never
 * verified.
 */
class OperatorNoticeNotificationUseCase(private val state: WarrenNoticeState) :
    InAppNotificationUseCase {

    override operator fun invoke(): Flow<InAppNotification?> =
        state.notices
            .map { notices -> notices.firstOrNull()?.let(InAppNotification::OperatorNotice) }
            .distinctUntilChanged()
}
