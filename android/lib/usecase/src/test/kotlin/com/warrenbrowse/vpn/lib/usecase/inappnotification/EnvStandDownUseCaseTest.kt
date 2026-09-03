package com.warrenbrowse.vpn.lib.usecase.inappnotification

import app.cash.turbine.test
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.repository.WarrenEnvStandDown
import com.warrenbrowse.vpn.lib.repository.WarrenEnvStandDownStore
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertNull
import org.junit.jupiter.api.extension.ExtendWith

@ExtendWith(TestCoroutineRule::class)
class EnvStandDownUseCaseTest {

    /** Stands in for the preferences file, so a new instance reads what the previous one wrote. */
    private val store =
        object : WarrenEnvStandDownStore {
            var record = WarrenEnvStandDown()

            override fun readEnvStandDown(): WarrenEnvStandDown = record

            override fun writeEnvStandDown(record: WarrenEnvStandDown) {
                this.record = record
            }
        }

    private var productionInstalled = false
    private var autoConnect = true
    private var blockingPolicy = true
    private val applied = mutableListOf<String>()

    /** A fresh instance over the same store: what the next process start builds. */
    private fun newUseCase() =
        EnvStandDownUseCase(
            store = store,
            higherEnvironmentInstalled = { productionInstalled },
            stopTunnel = { applied += "stopTunnel" },
            autoConnect =
                StandDownSetting(
                    read = { autoConnect },
                    write = {
                        autoConnect = it
                        applied += "autoConnect=$it"
                    },
                ),
            blockingPolicy =
                StandDownSetting(
                    read = { blockingPolicy },
                    write = {
                        blockingPolicy = it
                        applied += "blockingPolicy=$it"
                    },
                ),
        )

    @Test
    fun `a device without the production install is left alone`() = runTest {
        val standDown = newUseCase()
        standDown.refresh()

        standDown().test { assertNull(awaitItem()) }
        assertEquals(emptyList(), applied)
    }

    @Test
    fun `the first detection takes the tunnel down before it lifts the block`() = runTest {
        productionInstalled = true
        val standDown = newUseCase()

        standDown.refresh()

        // The order is the safety, and it is the desktop daemon's order: lifting
        // the block first would leave the device with no tunnel and no block for
        // the whole teardown.
        assertEquals(listOf("stopTunnel", "blockingPolicy=false", "autoConnect=false"), applied)
        standDown().test { assertEquals(InAppNotification.EnvStandDown, awaitItem()) }
    }

    @Test
    fun `a later start does not stand down again for the same install`() = runTest {
        productionInstalled = true
        newUseCase().refresh()
        applied.clear()

        val nextStart = newUseCase()
        nextStart.refresh()

        assertEquals(emptyList(), applied)
        nextStart().test { assertEquals(InAppNotification.EnvStandDown, awaitItem()) }
    }

    @Test
    fun `the manual re-enable puts the auto-connect and the block back`() = runTest {
        productionInstalled = true
        val standDown = newUseCase()
        standDown.refresh()
        applied.clear()

        standDown.reEnable()

        assertEquals(listOf("blockingPolicy=true", "autoConnect=true"), applied)
        standDown().test { assertNull(awaitItem()) }
    }

    @Test
    fun `the manual re-enable survives a restart while the install stays`() = runTest {
        productionInstalled = true
        val standDown = newUseCase()
        standDown.refresh()
        standDown.reEnable()
        applied.clear()

        val nextStart = newUseCase()
        nextStart.refresh()

        assertEquals(emptyList(), applied)
        nextStart().test { assertNull(awaitItem()) }
    }

    @Test
    fun `a production install removed and put back is a new transition`() = runTest {
        productionInstalled = true
        val first = newUseCase()
        first.refresh()
        first.reEnable()

        productionInstalled = false
        newUseCase().refresh()
        applied.clear()

        productionInstalled = true
        val afterReinstall = newUseCase()
        afterReinstall.refresh()

        assertEquals(listOf("stopTunnel", "blockingPolicy=false", "autoConnect=false"), applied)
        afterReinstall().test { assertEquals(InAppNotification.EnvStandDown, awaitItem()) }
    }

    @Test
    fun `the production install going away puts the auto-connect and the block back`() = runTest {
        productionInstalled = true
        val standDown = newUseCase()
        standDown.refresh()
        applied.clear()

        productionInstalled = false
        val afterUninstall = newUseCase()
        afterUninstall.refresh()

        // The record about to be dropped is the only place those two values are
        // written down. Dropping it without restoring them leaves the user with
        // no kill switch, no auto-connect, no banner, and nothing on screen that
        // ever said they had been turned off.
        assertEquals(listOf("blockingPolicy=true", "autoConnect=true"), applied)
        assertTrue(autoConnect)
        assertTrue(blockingPolicy)
        afterUninstall().test { assertNull(awaitItem()) }
    }

    @Test
    fun `an uninstall after the manual re-enable keeps what the user has chosen since`() = runTest {
        productionInstalled = true
        val standDown = newUseCase()
        standDown.refresh()
        standDown.reEnable()
        // The re-enable already put both values back, and the user has turned
        // them off themselves since. Writing the record again would overwrite
        // that choice.
        autoConnect = false
        blockingPolicy = false
        applied.clear()

        productionInstalled = false
        newUseCase().refresh()

        assertEquals(emptyList(), applied)
    }

    @Test
    fun `the stand-down outranks every connection-state banner`() {
        // It explains why this build will not connect at all, so a banner about
        // the connection it is not attempting must never sit on top of it.
        assertTrue(InAppNotification.EnvStandDown.priority > InAppNotification.HostOffline.priority)
    }
}
