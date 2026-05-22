package com.warrenbrowse.vpn.app

import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.shareIn
import com.warrenbrowse.vpn.core.NavKey2

/**
 * Process-wide observer hooked into [MainActivity]'s lifecycle. Owns
 * the back-stack snapshot used by the splash decision tree (cf.
 * `WarrenApp.kt`) and exposes a [DaemonScreenEvent] side-effect flow.
 *
 * D.4 step 19: the legacy Mullvad daemon-connectivity overlay has been
 * removed. On Warren mobile the gRPC management service does not exist;
 * the prior implementation kept emitting "Show NoDaemonScreen" on every
 * resume, blocking the rest of the UI. The flow now stays empty (the
 * subscriber in `WarrenApp.kt` is left in place so a future Warren-
 * native "service unreachable" overlay can plug in via the same wire).
 */
class WarrenAppViewModel : ViewModel(), LifecycleEventObserver {

    private val backStackFlow: MutableStateFlow<List<NavKey2>> = MutableStateFlow(emptyList())

    fun setCurrentBackStack(backStack: List<NavKey2>) {
        backStackFlow.value = backStack
    }

    /**
     * Empty stream today. Kept as a [SharedFlow] so the subscriber in
     * `WarrenApp.kt` still compiles and a future Warren-native overlay
     * (exit unreachable, wallet locked, etc.) can swap the underlying
     * source without ceremony.
     */
    val uiSideEffect: SharedFlow<DaemonScreenEvent> =
        flowOf<DaemonScreenEvent>().shareIn(viewModelScope, SharingStarted.Eagerly)

    override fun onStateChanged(source: LifecycleOwner, event: Lifecycle.Event) {
        // No-op on Warren: the daemon-state machine that consumed
        // lifecycle events is gone. Kept as an explicit override to
        // satisfy [LifecycleEventObserver] in case the activity hook is
        // re-added later.
    }
}

sealed interface DaemonScreenEvent {
    data object Show : DaemonScreenEvent

    data object Remove : DaemonScreenEvent
}
