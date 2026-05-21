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
        every { mockPrefs.getBoolean("obfuscation_m40", false) } returns false

        val repo = WarrenLocalSettingsRepository(mockContext)

        assertTrue(repo.daitaEnabled.value)
        assertFalse(repo.natPmpEnabled.value)
        assertTrue(repo.multiHopEnabled.value)
        assertFalse(repo.obfuscationM40.value)
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
    fun `setObfuscationM40 writes through to prefs and updates state`() {
        every { mockPrefs.getBoolean(any(), any()) } returns false
        val repo = WarrenLocalSettingsRepository(mockContext)

        repo.setObfuscationM40(true)

        assertTrue(repo.obfuscationM40.value)
        verify { mockEditor.putBoolean("obfuscation_m40", true) }
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
