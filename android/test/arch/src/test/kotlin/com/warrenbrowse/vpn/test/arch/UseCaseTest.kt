package com.warrenbrowse.vpn.test.arch

import com.lemonappdev.konsist.api.Konsist
import com.lemonappdev.konsist.api.verify.assertTrue
import org.junit.jupiter.api.Test

class UseCaseTest {
    @Test
    fun `ensure all use cases end with UseCase suffix`() =
        allUseCasesFiles().assertTrue { it.name.endsWith("UseCase") }

    @Test
    fun `ensure every use cases is public`() =
        allUseCases().assertTrue { it.hasPublicOrDefaultModifier }

    private fun allUseCasesFiles() =
        Konsist.scopeFromProduction().files.filter { it.resideInPath("..usecase..") }

    private fun allUseCases() =
        Konsist.scopeFromProduction()
            .classes()
            .filter { it.resideInPackage("..usecase..") }
            .filter { !it.hasPrivateModifier }
}
