package com.warrenbrowse.vpn.lib.repository

import android.content.Context
import android.content.SharedPreferences
import io.mockk.every
import io.mockk.mockk
import io.mockk.slot
import io.mockk.verify
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test

class WarrenLocalSettingsRepositoryTest {

    private val mockContext: Context = mockk()
    private val mockAppContext: Context = mockk()
    private val mockPrefs: SharedPreferences = mockk(relaxed = true)
    private val mockEditor: SharedPreferences.Editor = mockk(relaxed = true)

    @BeforeEach
    fun setUp() {
        every { mockContext.applicationContext } returns mockAppContext
        every { mockAppContext.getSharedPreferences(any(), any()) } returns mockPrefs
        every { mockPrefs.edit() } returns mockEditor
        every { mockEditor.putBoolean(any(), any()) } returns mockEditor
        every { mockEditor.putString(any(), any()) } returns mockEditor
        every { mockEditor.putInt(any(), any()) } returns mockEditor
        every { mockEditor.remove(any()) } returns mockEditor
        // Default seed for the selected-exit-id read on construction;
        // individual tests can override.
        every { mockPrefs.getString(any(), any()) } returns null
    }

    @Test
    fun `state flows seed from disk on construction`() {
        every { mockPrefs.getBoolean("daita_enabled", false) } returns true
        every { mockPrefs.getBoolean("nat_pmp_enabled", false) } returns false
        every { mockPrefs.getBoolean("multi_hop_enabled", false) } returns true

        val repo = WarrenLocalSettingsRepository(mockContext)

        assertTrue(repo.daitaEnabled.value)
        assertFalse(repo.natPmpEnabled.value)
        assertTrue(repo.multiHopEnabled.value)
    }

    @Test
    fun `setDaitaEnabled writes through to prefs and updates state`() {
        every { mockPrefs.getBoolean(any(), any()) } returns false
        val repo = WarrenLocalSettingsRepository(mockContext)

        repo.setDaitaEnabled(true)

        assertTrue(repo.daitaEnabled.value)
        verify { mockEditor.putBoolean("daita_enabled", true) }
        verify { mockEditor.apply() }
    }

    @Test
    fun `setNatPmpEnabled writes through to prefs and updates state`() {
        every { mockPrefs.getBoolean(any(), any()) } returns false
        val repo = WarrenLocalSettingsRepository(mockContext)

        repo.setNatPmpEnabled(true)

        assertTrue(repo.natPmpEnabled.value)
        verify { mockEditor.putBoolean("nat_pmp_enabled", true) }
    }

    @Test
    fun `setMultiHopEnabled writes through to prefs and updates state`() {
        every { mockPrefs.getBoolean(any(), any()) } returns true
        val repo = WarrenLocalSettingsRepository(mockContext)

        repo.setMultiHopEnabled(false)

        assertFalse(repo.multiHopEnabled.value)
        verify { mockEditor.putBoolean("multi_hop_enabled", false) }
    }

    @Test
    fun `selectedExitId round-trips through prefs`() {
        every { mockPrefs.getBoolean(any(), any()) } returns false
        every { mockPrefs.getString("selected_exit_id", null) } returns "2921abad869e94064b56cf48c8da3631"
        every { mockEditor.putString(any(), any()) } returns mockEditor
        every { mockEditor.remove(any()) } returns mockEditor

        val repo = WarrenLocalSettingsRepository(mockContext)
        assertEquals("2921abad869e94064b56cf48c8da3631", repo.selectedExitId.value)

        repo.setSelectedExitId("ffffffffffffffffffffffffffffffff")
        assertEquals("ffffffffffffffffffffffffffffffff", repo.selectedExitId.value)
        verify { mockEditor.putString("selected_exit_id", "ffffffffffffffffffffffffffffffff") }

        repo.setSelectedExitId(null)
        assertEquals(null, repo.selectedExitId.value)
        verify { mockEditor.remove("selected_exit_id") }
    }

    @Test
    fun `ipv6 and lockdown default to false and write through`() {
        every { mockPrefs.getBoolean(any(), any()) } returns false
        val repo = WarrenLocalSettingsRepository(mockContext)

        assertFalse(repo.ipv6Enabled.value)
        assertFalse(repo.lockdownMode.value)

        repo.setIpv6Enabled(true)
        repo.setLockdownMode(true)

        assertTrue(repo.ipv6Enabled.value)
        assertTrue(repo.lockdownMode.value)
        verify { mockEditor.putBoolean("ipv6_enabled", true) }
        verify { mockEditor.putBoolean("lockdown_mode", true) }
    }

    @Test
    fun `dns state defaults to default and normalizes unknown values`() {
        every { mockPrefs.getBoolean(any(), any()) } returns false
        val repo = WarrenLocalSettingsRepository(mockContext)

        assertEquals(WarrenLocalSettingsRepository.DNS_STATE_DEFAULT, repo.dnsState.value)

        repo.setDnsState(WarrenLocalSettingsRepository.DNS_STATE_CUSTOM)
        assertEquals(WarrenLocalSettingsRepository.DNS_STATE_CUSTOM, repo.dnsState.value)
        verify { mockEditor.putString("dns_state", "custom") }

        repo.setDnsState("garbage")
        assertEquals(WarrenLocalSettingsRepository.DNS_STATE_DEFAULT, repo.dnsState.value)
    }

    @Test
    fun `custom dns servers round-trip and drop blanks`() {
        every { mockPrefs.getBoolean(any(), any()) } returns false
        every { mockPrefs.getString("dns_custom_servers", null) } returns "9.9.9.9,149.112.112.112"
        val repo = WarrenLocalSettingsRepository(mockContext)

        assertEquals(listOf("9.9.9.9", "149.112.112.112"), repo.customDnsServers.value)

        repo.setCustomDnsServers(listOf("1.1.1.1", "  ", "", "8.8.8.8"))
        assertEquals(listOf("1.1.1.1", "8.8.8.8"), repo.customDnsServers.value)
        verify { mockEditor.putString("dns_custom_servers", "1.1.1.1,8.8.8.8") }
    }

    @Test
    fun `content blocking toggles write through`() {
        every { mockPrefs.getBoolean(any(), any()) } returns false
        val repo = WarrenLocalSettingsRepository(mockContext)

        repo.setBlockAds(true)
        repo.setBlockMalware(true)

        assertTrue(repo.blockAds.value)
        assertTrue(repo.blockMalware.value)
        assertFalse(repo.blockTrackers.value)
        verify { mockEditor.putBoolean("dns_block_ads", true) }
        verify { mockEditor.putBoolean("dns_block_malware", true) }
    }

    @Test
    fun `recents are most-recent-first, deduped and capped at five`() {
        every { mockPrefs.getBoolean(any(), any()) } returns false
        every { mockPrefs.getBoolean("recents_enabled", true) } returns true
        val repo = WarrenLocalSettingsRepository(mockContext)

        listOf("a", "b", "c", "a").forEach { repo.recordRecentExit(it) }
        // "a" deduped to front, order reflects last-touched.
        assertEquals(listOf("a", "c", "b"), repo.recentExitIds.value)

        repo.recordRecentExit("d")
        repo.recordRecentExit("e")
        repo.recordRecentExit("f")
        // Capped at five, oldest ("b") evicted.
        assertEquals(listOf("f", "e", "d", "a", "c"), repo.recentExitIds.value)
    }

    @Test
    fun `selecting an exit records it as recent, clearing does not`() {
        every { mockPrefs.getBoolean(any(), any()) } returns false
        every { mockPrefs.getBoolean("recents_enabled", true) } returns true
        every { mockPrefs.getString("selected_exit_id", null) } returns null
        val repo = WarrenLocalSettingsRepository(mockContext)

        repo.setSelectedExitId("exit-1")
        assertEquals(listOf("exit-1"), repo.recentExitIds.value)

        repo.setSelectedExitId(null)
        // Clearing the selection leaves recents untouched.
        assertEquals(listOf("exit-1"), repo.recentExitIds.value)
    }

    @Test
    fun `setTunnelMtu clamps to the safe range`() {
        every { mockPrefs.getBoolean(any(), any()) } returns false
        every { mockPrefs.getInt(any(), any()) } returns WarrenLocalSettingsRepository.MTU_MAX
        val repo = WarrenLocalSettingsRepository(mockContext)

        repo.setTunnelMtu(1000)
        assertEquals(1000, repo.tunnelMtu.value)
        // Above the max is clamped down.
        repo.setTunnelMtu(5000)
        assertEquals(WarrenLocalSettingsRepository.MTU_MAX, repo.tunnelMtu.value)
        // Below the min is clamped up.
        repo.setTunnelMtu(0)
        assertEquals(WarrenLocalSettingsRepository.MTU_MIN, repo.tunnelMtu.value)
    }

    @Test
    fun `disabling recents stops recording and clears the existing list`() {
        every { mockPrefs.getBoolean(any(), any()) } returns false
        every { mockPrefs.getBoolean("recents_enabled", true) } returns true
        val repo = WarrenLocalSettingsRepository(mockContext)

        repo.recordRecentExit("a")
        assertEquals(listOf("a"), repo.recentExitIds.value)

        // Turning recents off forgets the current list immediately...
        repo.setRecentsEnabled(false)
        assertEquals(emptyList<String>(), repo.recentExitIds.value)
        assertFalse(repo.recentsEnabled.value)

        // ...and no new recents are recorded while it stays off.
        repo.recordRecentExit("b")
        assertEquals(emptyList<String>(), repo.recentExitIds.value)

        // Re-enabling resumes recording.
        repo.setRecentsEnabled(true)
        repo.recordRecentExit("c")
        assertEquals(listOf("c"), repo.recentExitIds.value)
    }

    @Test
    fun `entry and exit country normalize to uppercase ISO-2 or clear`() {
        every { mockPrefs.getBoolean(any(), any()) } returns false
        val repo = WarrenLocalSettingsRepository(mockContext)

        repo.setExitCountry("fr")
        assertEquals("FR", repo.exitCountry.value)
        verify { mockEditor.putString("exit_country", "FR") }

        repo.setEntryCountry("Deu") // not 2 letters -> cleared
        assertEquals(null, repo.entryCountry.value)

        repo.setExitCountry(null)
        assertEquals(null, repo.exitCountry.value)
        verify { mockEditor.remove("exit_country") }
    }

    @Test
    fun `nat-pmp protocol normalizes and writes through`() {
        every { mockPrefs.getBoolean(any(), any()) } returns false
        val repo = WarrenLocalSettingsRepository(mockContext)

        assertEquals("udp", repo.natPmpProtocol.value)
        repo.setNatPmpProtocol("tcp")
        assertEquals("tcp", repo.natPmpProtocol.value)
        verify { mockEditor.putString("nat_pmp_protocol", "tcp") }

        repo.setNatPmpProtocol("garbage")
        assertEquals("udp", repo.natPmpProtocol.value)
    }

    @Test
    fun `nat-pmp external port clamps to the dynamic range or zero`() {
        every { mockPrefs.getBoolean(any(), any()) } returns false
        val repo = WarrenLocalSettingsRepository(mockContext)

        repo.setNatPmpExternalPort(51820)
        assertEquals(51820, repo.natPmpExternalPort.value)
        verify { mockEditor.putInt("nat_pmp_external_port", 51820) }

        repo.setNatPmpExternalPort(80) // below dynamic range -> auto (0)
        assertEquals(0, repo.natPmpExternalPort.value)

        repo.setNatPmpExternalPort(-1)
        assertEquals(0, repo.natPmpExternalPort.value)
    }

    @Test
    fun `nat-pmp lifetime clamps to bounds`() {
        every { mockPrefs.getBoolean(any(), any()) } returns false
        val repo = WarrenLocalSettingsRepository(mockContext)

        repo.setNatPmpLifetimeSecs(21_600)
        assertEquals(21_600, repo.natPmpLifetimeSecs.value)

        repo.setNatPmpLifetimeSecs(10) // below min
        assertEquals(WarrenLocalSettingsRepository.NAT_PMP_MIN_LIFETIME_SECS, repo.natPmpLifetimeSecs.value)

        repo.setNatPmpLifetimeSecs(999_999) // above max
        assertEquals(WarrenLocalSettingsRepository.NAT_PMP_MAX_LIFETIME_SECS, repo.natPmpLifetimeSecs.value)
    }

    @Test
    fun `flows emit current value to new collectors`() {
        every { mockPrefs.getBoolean(any(), any()) } returns false
        val repo = WarrenLocalSettingsRepository(mockContext)

        repo.setDaitaEnabled(true)
        val capturedKey = slot<String>()
        val capturedValue = slot<Boolean>()
        verify { mockEditor.putBoolean(capture(capturedKey), capture(capturedValue)) }
        assertEquals("daita_enabled", capturedKey.captured)
        assertTrue(capturedValue.captured)
    }
}
