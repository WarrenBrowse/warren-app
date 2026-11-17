package com.warrenbrowse.vpn.di

import com.warrenbrowse.vpn.feature.language.impl.LanguageRepository
import com.warrenbrowse.vpn.feature.language.impl.LanguageViewModel
import kotlin.reflect.KClass
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import org.koin.core.annotation.KoinInternalApi
import org.koin.dsl.koinApplication

/**
 * The settings offer the language row on every supported release (AppCompat
 * carries the per-app language below API 33), so the screen behind it must
 * resolve on every release too. The JVM test runtime reports an SDK level below
 * every Android release, which is the oldest device this graph can meet.
 */
@OptIn(KoinInternalApi::class)
class LanguageGraphTest {
    private fun declares(type: KClass<*>): Boolean {
        val koin =
            koinApplication(createEagerInstances = false) { modules(appModule, uiModule) }.koin
        return koin.instanceRegistry.instances.values.any {
            it.beanDefinition.primaryType == type
        }
    }

    @Test
    fun `the language screen resolves its view model below API 33`() {
        assertTrue(declares(LanguageViewModel::class))
    }

    @Test
    fun `the language screen resolves its repository below API 33`() {
        assertTrue(declares(LanguageRepository::class))
    }
}
