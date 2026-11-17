package com.warrenbrowse.vpn.test.arch.classes

import com.lemonappdev.konsist.api.Konsist
import com.lemonappdev.konsist.api.verify.assertTrue
import org.junit.jupiter.api.Test

class ClassTest {
    @Test
    fun `ensure that all view model test classes are annotated with TestCoroutineRule`() =
        Konsist.scopeFromTest()
            .classes(includeNested = false)
            .filter {
                // Only include classes that are view model tests
                // We want to ignore the class "ViewModelTest" which contains konsist tests
                it.name.contains(".+ViewModelTest".toRegex())
            }
            .assertTrue {
                it.hasAnnotation { annotation ->
                    annotation.name == "ExtendWith" &&
                        annotation.arguments.any { argument ->
                            argument.value == "TestCoroutineRule::class"
                        }
                }
            }
}
