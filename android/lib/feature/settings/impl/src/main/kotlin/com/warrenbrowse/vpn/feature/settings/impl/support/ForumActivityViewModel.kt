package com.warrenbrowse.vpn.feature.settings.impl.support

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.warrenbrowse.vpn.lib.common.constant.VIEW_MODEL_STOP_TIMEOUT
import com.warrenbrowse.vpn.lib.model.forum.ForumNotification
import com.warrenbrowse.vpn.lib.repository.ForumIdentityRepository
import com.warrenbrowse.vpn.lib.repository.ForumNotificationsReader
import com.warrenbrowse.vpn.lib.repository.ForumNotificationsResult
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.WhileSubscribed
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** What the activity panel shows (desktop `ForumActivityState`). */
sealed interface ForumActivityUiState {
    data object Loading : ForumActivityUiState

    data class Ready(val notifications: List<ForumNotification>) : ForumActivityUiState {
        val hasUnread: Boolean
            get() = notifications.any { it.unread }
    }

    data object Error : ForumActivityUiState
}

/**
 * Reads the user's own forum notifications once, when the panel opens.
 *
 * Deliberately not a subscription and not a poll: the header badge already
 * comes from the broadcast digest, which asks the server nothing about
 * anybody. This is the only request tied to the account, and it happens only
 * because the user asked to see the content.
 */
class ForumActivityViewModel(
    private val reader: ForumNotificationsReader,
    forumIdentity: ForumIdentityRepository,
) : ViewModel() {
    private val _state = MutableStateFlow<ForumActivityUiState>(ForumActivityUiState.Loading)
    val state: StateFlow<ForumActivityUiState> = _state.asStateFlow()

    /** The name this wallet posts under, for the empty panel. */
    val handle: StateFlow<String?> =
        forumIdentity.identity
            .map { it?.handle }
            .stateIn(
                viewModelScope,
                SharingStarted.WhileSubscribed(VIEW_MODEL_STOP_TIMEOUT),
                forumIdentity.identity.value?.handle,
            )

    init {
        load()
    }

    fun reload() = load()

    private fun load() {
        _state.value = ForumActivityUiState.Loading
        viewModelScope.launch {
            _state.value =
                when (val result = reader.list()) {
                    is ForumNotificationsResult.Ok -> ForumActivityUiState.Ready(result.notifications)
                    is ForumNotificationsResult.Error -> ForumActivityUiState.Error
                }
        }
    }

    /**
     * Repainted before the round trip: the forum is a network away and the
     * user has already decided. A failure leaves the cards read here while
     * the next digest puts them back, which is the harmless direction.
     */
    fun markAllRead() {
        _state.update { current ->
            if (current is ForumActivityUiState.Ready) {
                ForumActivityUiState.Ready(current.notifications.map { it.copy(unread = false) })
            } else {
                current
            }
        }
        viewModelScope.launch { reader.markSeen() }
    }

    /**
     * Opening the post is what marks it read on the forum, and that happens in
     * the browser a moment later. This only stops the card from claiming
     * otherwise in the meantime.
     */
    fun markOneRead(id: Long) {
        _state.update { current ->
            if (current is ForumActivityUiState.Ready) {
                ForumActivityUiState.Ready(
                    current.notifications.map { if (it.id == id) it.copy(unread = false) else it }
                )
            } else {
                current
            }
        }
    }
}
