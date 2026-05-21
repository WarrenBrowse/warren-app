package com.warrenbrowse.vpn.feature.apiaccess.impl.screen.delete

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.WhileSubscribed
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import com.warrenbrowse.vpn.lib.common.constant.VIEW_MODEL_STOP_TIMEOUT
import com.warrenbrowse.vpn.lib.model.ApiAccessMethodId
import com.warrenbrowse.vpn.lib.model.RemoveApiAccessMethodError
import com.warrenbrowse.vpn.lib.repository.ApiAccessRepository

class DeleteApiAccessMethodConfirmationViewModel(
    private val apiAccessMethodId: ApiAccessMethodId,
    private val apiAccessRepository: ApiAccessRepository,
) : ViewModel() {

    private val _uiSideEffect =
        Channel<DeleteApiAccessMethodConfirmationSideEffect>(Channel.BUFFERED)
    val uiSideEffect = _uiSideEffect.receiveAsFlow()

    private val _error = MutableStateFlow<RemoveApiAccessMethodError?>(null)

    val uiState =
        _error
            .map { DeleteApiAccessMethodUiState(it) }
            .stateIn(
                viewModelScope,
                SharingStarted.WhileSubscribed(VIEW_MODEL_STOP_TIMEOUT),
                DeleteApiAccessMethodUiState(null),
            )

    fun deleteApiAccessMethod() {
        viewModelScope.launch {
            _error.emit(null)
            apiAccessRepository
                .removeApiAccessMethod(apiAccessMethodId)
                .fold(
                    { _error.tryEmit(it) },
                    { _uiSideEffect.send(DeleteApiAccessMethodConfirmationSideEffect.Deleted) },
                )
        }
    }
}

sealed interface DeleteApiAccessMethodConfirmationSideEffect {
    data object Deleted : DeleteApiAccessMethodConfirmationSideEffect
}
