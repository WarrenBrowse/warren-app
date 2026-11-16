package com.warrenbrowse.vpn.di

import com.warrenbrowse.vpn.lib.usecase.DeviceRevocationUseCase
import com.warrenbrowse.vpn.lib.usecase.WarrenLogoutUseCase
import kotlin.reflect.KClass
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import org.koin.core.annotation.KoinInternalApi
import org.koin.dsl.koinApplication

/**
 * The VPN service runs without the activity whenever Android starts it alone:
 * after an update while connected, or at boot under always-on. It then has
 * only `appModule` and `vpnServiceModule`, never the activity's `uiModule`, so
 * everything it resolves must be declared in those two.
 */
@OptIn(KoinInternalApi::class)
class ServiceGraphTest {
    private fun declaresWithoutTheActivity(type: KClass<*>): Boolean {
        // Declarations only: nothing is instantiated, so no Android runtime is needed.
        val koin =
            koinApplication(createEagerInstances = false) { modules(appModule, vpnServiceModule) }
                .koin
        return koin.instanceRegistry.instances.values.any {
            it.beanDefinition.primaryType == type
        }
    }

    @Test
    fun `the service can resolve the revocation watch without the activity`() {
        assertTrue(declaresWithoutTheActivity(DeviceRevocationUseCase::class))
    }

    @Test
    fun `the service can resolve the logout the revocation runs`() {
        assertTrue(declaresWithoutTheActivity(WarrenLogoutUseCase::class))
    }
}
