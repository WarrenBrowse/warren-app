package com.warrenbrowse.vpn.feature.login.impl

import com.warrenbrowse.vpn.lib.model.ErrorState
import com.warrenbrowse.vpn.lib.model.ErrorStateCause
import com.warrenbrowse.vpn.lib.model.TunnelState
import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.junit.jupiter.api.Test

class LoginBlockNoticeTest {

    private fun blocking() =
        TunnelState.Error(ErrorState(ErrorStateCause.WarrenKillSwitchActive, isBlocking = true))

    @Test
    fun `a blocked tunnel under lockdown names lockdown mode`() {
        assertEquals(
            LoginBlockNotice.LockdownMode,
            loginBlockNotice(blocking(), lockdownMode = true),
        )
    }

    @Test
    fun `a blocked tunnel without lockdown names the kill switch`() {
        assertEquals(
            LoginBlockNotice.KillSwitch,
            loginBlockNotice(blocking(), lockdownMode = false),
        )
    }

    @Test
    fun `a released error state raises nothing`() {
        val released =
            TunnelState.Error(
                ErrorState(ErrorStateCause.WarrenKillSwitchActive, isBlocking = false)
            )
        assertNull(loginBlockNotice(released, lockdownMode = true))
    }

    @Test
    fun `a tunnel that is not in error raises nothing, lockdown or not`() {
        assertNull(loginBlockNotice(TunnelState.Disconnected(), lockdownMode = true))
        assertNull(
            loginBlockNotice(
                TunnelState.Connecting(
                    endpoint = null,
                    location = null,
                    featureIndicators = emptyList(),
                ),
                lockdownMode = true,
            )
        )
    }
}
