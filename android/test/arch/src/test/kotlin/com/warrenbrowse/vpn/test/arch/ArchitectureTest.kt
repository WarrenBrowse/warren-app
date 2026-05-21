package com.warrenbrowse.vpn.test.arch

import com.lemonappdev.konsist.api.Konsist
import com.lemonappdev.konsist.api.architecture.KoArchitectureCreator.assertArchitecture
import com.lemonappdev.konsist.api.architecture.Layer
import org.junit.jupiter.api.Test

class ArchitectureTest {

    @Test
    fun `ensure model layer depends on nothing`() =
        Konsist.scopeFromProduction().assertArchitecture {
            val model = Layer("Model", "com.warrenbrowse.vpn.lib.model..")

            model.dependsOnNothing()
        }
}
