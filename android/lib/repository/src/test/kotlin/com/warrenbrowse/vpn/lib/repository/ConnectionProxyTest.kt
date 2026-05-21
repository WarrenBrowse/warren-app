package com.warrenbrowse.vpn.lib.repository

import android.content.Context
import android.content.Intent
import arrow.core.left
import arrow.core.right
import io.mockk.coVerify
import io.mockk.every
import io.mockk.mockk
import io.mockk.mockkStatic
import io.mockk.unmockkAll
import kotlinx.coroutines.test.runTest
import com.warrenbrowse.vpn.lib.common.util.prepareVpnSafe
import com.warrenbrowse.vpn.lib.grpc.ManagementService
import com.warrenbrowse.vpn.lib.model.PrepareError
import com.warrenbrowse.vpn.lib.model.Prepared
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test

class ConnectionProxyTest {

    private val mockContext: Context = mockk()
    private val mockManagementService: ManagementService = mockk(relaxed = true)
    private val mockTranslationRepository: RelayLocationTranslationRepository =
        mockk(relaxed = true)

    private val connectionProxy: ConnectionProxy =
        ConnectionProxy(
            context = mockContext,
            managementService = mockManagementService,
            translationRepository = mockTranslationRepository,
        )

    @BeforeEach
    fun setup() {
        mockkStatic(VPN_SERVICE_UTILS)
    }

    @Test
    fun `connect with vpn permission allowed should call managementService connect`() = runTest {
        every { mockContext.prepareVpnSafe() } returns Prepared.right()
        connectionProxy.connect()
        coVerify(exactly = 1) { mockManagementService.connect() }
    }

    @Test
    fun `connect with vpn permission not allowed should not call managementService connect`() =
        runTest {
            every { mockContext.prepareVpnSafe() } returns PrepareError.NotPrepared(Intent()).left()
            connectionProxy.connect()
            coVerify(exactly = 0) { mockManagementService.connect() }
        }

    @Test
    fun `disconnect should call managementService disconnect`() = runTest {
        connectionProxy.disconnect(mockk())
        coVerify(exactly = 1) { mockManagementService.disconnect(any()) }
    }

    @Test
    fun `reconnect should call managementService reconnect`() = runTest {
        connectionProxy.reconnect()
        coVerify(exactly = 1) { mockManagementService.reconnect() }
    }

    @AfterEach
    fun tearDown() {
        unmockkAll()
    }

    companion object {
        const val VPN_SERVICE_UTILS = "com.warrenbrowse.vpn.lib.common.util.VpnServiceUtilsKt"
    }
}
