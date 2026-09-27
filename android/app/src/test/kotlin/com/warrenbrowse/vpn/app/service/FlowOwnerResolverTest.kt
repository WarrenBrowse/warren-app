package com.warrenbrowse.vpn.app.service

import android.content.pm.PackageManager
import android.net.ConnectivityManager
import io.mockk.every
import io.mockk.mockk
import java.net.InetAddress
import java.net.InetSocketAddress
import org.junit.jupiter.api.Assertions.assertArrayEquals
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

/**
 * The two platform calls the engine attributes a "Country per app" flow with. The platform is the
 * system boundary, so it is faked; what is tested is that the flow reaches it as the TUN carried
 * it, and that every refusal reads as "no owner" rather than failing the packet path.
 */
class FlowOwnerResolverTest {
    private val connectivity = mockk<ConnectivityManager>()
    private val packages = mockk<PackageManager>()
    private val resolver = FlowOwnerResolver(connectivity, packages)

    private val tun = byteArrayOf(10, 64, 0, 1)
    private val remote = byteArrayOf(198.toByte(), 51, 100, 9)

    @Test
    fun `asks the platform for the owner of the flow as the tun carries it`() {
        every {
            connectivity.getConnectionOwnerUid(
                TCP,
                InetSocketAddress(InetAddress.getByAddress(tun), 40_001),
                InetSocketAddress(InetAddress.getByAddress(remote), 443),
            )
        } returns 10_123

        assertEquals(10_123, resolver.ownerUid(TCP, tun, 40_001, remote, 443))
    }

    @Test
    fun `a refused lookup reads as no owner`() {
        every { connectivity.getConnectionOwnerUid(any(), any(), any()) } throws
            SecurityException("not the active VPN")

        assertEquals(-1, resolver.ownerUid(UDP, tun, 40_002, remote, 53))
    }

    @Test
    fun `an address the platform cannot read reads as no owner`() {
        assertEquals(-1, resolver.ownerUid(TCP, byteArrayOf(1, 2, 3), 1, remote, 443))
    }

    @Test
    fun `names the packages of a uid, or none`() {
        every { packages.getPackagesForUid(10_123) } returns arrayOf("org.browser")
        every { packages.getPackagesForUid(10_999) } returns null

        assertArrayEquals(arrayOf("org.browser"), resolver.packagesForUid(10_123))
        assertNull(resolver.packagesForUid(10_999))
    }

    private companion object {
        const val TCP = 6
        const val UDP = 17
    }
}
