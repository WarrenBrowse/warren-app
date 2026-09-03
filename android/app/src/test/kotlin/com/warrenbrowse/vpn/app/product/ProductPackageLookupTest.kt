package com.warrenbrowse.vpn.app.product

import android.content.pm.PackageManager
import android.content.pm.PackageManager.NameNotFoundException
import io.mockk.every
import io.mockk.mockk
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

/**
 * The one system-boundary adapter of the Android stand-down. It exists because
 * `getPackageInfo` answers "absent" by throwing, and a thrown lookup left
 * uncaught would take the whole app start down instead of reporting that
 * nothing outranks this install.
 */
class ProductPackageLookupTest {

    private val packageManager: PackageManager = mockk()

    @Test
    fun `an installed application is reported present`() {
        every { packageManager.getPackageInfo(PROD_APPLICATION_ID, 0) } returns mockk()

        assertTrue(packageManager.isApplicationInstalled(PROD_APPLICATION_ID))
    }

    @Test
    fun `an absent or invisible application is reported absent, not thrown`() {
        // The same exception carries both cases: genuinely not installed, and
        // installed but undeclared in this flavor's manifest `<queries>`.
        every { packageManager.getPackageInfo(PROD_APPLICATION_ID, 0) } throws
            NameNotFoundException()

        assertFalse(packageManager.isApplicationInstalled(PROD_APPLICATION_ID))
    }
}
