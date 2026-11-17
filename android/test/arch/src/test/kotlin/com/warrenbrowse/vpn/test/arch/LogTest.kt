package com.warrenbrowse.vpn.test.arch

import com.lemonappdev.konsist.api.Konsist
import com.lemonappdev.konsist.api.ext.list.imports
import com.lemonappdev.konsist.api.verify.assertFalse
import java.util.stream.Stream
import org.junit.jupiter.api.DynamicTest
import org.junit.jupiter.api.TestFactory

class LogTest {
    @TestFactory
    fun `ensure no usage of disallowed loggers`(): Stream<DynamicTest> =
        DISALLOWED_LOGGER_PACKAGE_NAMES.stream().map { disallowedLoggerPackageName ->
            val testName = "ensure no usage of: $disallowedLoggerPackageName"
            DynamicTest.dynamicTest(testName) {
                Konsist.scopeFromProject()
                    .files
                    // The frame-timing summary goes to logcat on purpose and nowhere else: the
                    // app logger also writes the file a problem report carries.
                    .filterNot { it.resideInPath("..app/perf..") }
                    .imports
                    .assertFalse { import -> import.name == disallowedLoggerPackageName }
            }
        }

    companion object {
        private val DISALLOWED_LOGGER_PACKAGE_NAMES = listOf("android.util.Log")
    }
}
