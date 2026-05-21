package com.warrenbrowse.vpn.feature.apiaccess.impl.screen.save

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import com.warrenbrowse.vpn.feature.apiaccess.api.SaveApiAccessMethodNavKey
import com.warrenbrowse.vpn.lib.model.ApiAccessMethod
import com.warrenbrowse.vpn.lib.model.ApiAccessMethodId
import com.warrenbrowse.vpn.lib.model.ApiAccessMethodName
import com.warrenbrowse.vpn.lib.model.NewAccessMethodSetting
import com.warrenbrowse.vpn.lib.repository.ApiAccessRepository

class SaveApiAccessMethodViewModel(
    navArgs: SaveApiAccessMethodNavKey,
    private val apiAccessRepository: ApiAccessRepository,
) : ViewModel() {
    private val apiAccessMethodId: ApiAccessMethodId? = navArgs.id
    private val apiAccessMethodName: ApiAccessMethodName = navArgs.name
    private val customProxy: ApiAccessMethod.CustomProxy = navArgs.customProxy

    private val _uiSideEffect = Channel<SaveApiAccessMethodSideEffect>()
    val uiSideEffect = _uiSideEffect.receiveAsFlow()
    private val _uiState = MutableStateFlow(SaveApiAccessMethodUiState())
    val uiState: StateFlow<SaveApiAccessMethodUiState> = _uiState

    init {
        viewModelScope.launch {
            apiAccessRepository
                .testCustomApiAccessMethod(customProxy)
                .fold(
                    {
                        _uiState.update {
                            it.copy(testingState = TestApiAccessMethodState.Result.Failure)
                        }
                    },
                    {
                        _uiState.update {
                            it.copy(testingState = TestApiAccessMethodState.Result.Successful)
                        }
                        save()
                    },
                )
        }
    }

    fun save() {
        viewModelScope.launch {
            _uiState.update { it.copy(isSaving = true) }
            if (apiAccessMethodId != null) {
                updateAccessMethod(
                    id = apiAccessMethodId,
                    name = apiAccessMethodName,
                    apiAccessMethod = customProxy,
                )
            } else {
                addNewAccessMethod(
                    NewAccessMethodSetting(
                        name = apiAccessMethodName,
                        enabled = true,
                        apiAccessMethod = customProxy,
                    )
                )
            }
        }
    }

    private suspend fun addNewAccessMethod(newAccessMethodSetting: NewAccessMethodSetting) {
        apiAccessRepository
            .addApiAccessMethod(newAccessMethodSetting)
            .fold(
                { _uiSideEffect.send(SaveApiAccessMethodSideEffect.CouldNotSaveApiAccessMethod) },
                { _uiSideEffect.send(SaveApiAccessMethodSideEffect.SuccessfullyCreatedApiMethod) },
            )
    }

    private suspend fun updateAccessMethod(
        id: ApiAccessMethodId,
        name: ApiAccessMethodName,
        apiAccessMethod: ApiAccessMethod.CustomProxy,
    ) {
        apiAccessRepository
            .updateApiAccessMethod(
                apiAccessMethodId = id,
                apiAccessMethodName = name,
                apiAccessMethod = apiAccessMethod,
            )
            .fold(
                { _uiSideEffect.send(SaveApiAccessMethodSideEffect.CouldNotSaveApiAccessMethod) },
                { _uiSideEffect.send(SaveApiAccessMethodSideEffect.SuccessfullyCreatedApiMethod) },
            )
    }
}

sealed interface SaveApiAccessMethodSideEffect {
    data object SuccessfullyCreatedApiMethod : SaveApiAccessMethodSideEffect

    data object CouldNotSaveApiAccessMethod : SaveApiAccessMethodSideEffect
}
