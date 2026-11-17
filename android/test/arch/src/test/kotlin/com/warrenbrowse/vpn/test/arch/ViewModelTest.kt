package com.warrenbrowse.vpn.test.arch

import androidx.lifecycle.ViewModel
import com.lemonappdev.konsist.api.Konsist
import com.lemonappdev.konsist.api.ext.list.withAllParentsOf
import com.lemonappdev.konsist.api.verify.assertTrue
import org.junit.jupiter.api.Test

class ViewModelTest {
    @Test
    fun `ensure view models have view model suffix`() =
        allViewModels().assertTrue { it.name.endsWith("ViewModel") }

    private fun allViewModels() =
        Konsist.scopeFromProject().classes().withAllParentsOf(ViewModel::class)
}
