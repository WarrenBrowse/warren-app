package com.warrenbrowse.vpn.lib.repository

import androidx.datastore.core.DataStore
import com.warrenbrowse.vpn.lib.model.BuildVersion
import com.warrenbrowse.vpn.repository.UserPreferences
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

/** A DataStore held in memory: the file is the system boundary. */
private class InMemoryStore(initial: UserPreferences) : DataStore<UserPreferences> {
    private val state = MutableStateFlow(initial)
    override val data: Flow<UserPreferences> = state

    override suspend fun updateData(
        transform: suspend (t: UserPreferences) -> UserPreferences
    ): UserPreferences = transform(state.value).also { state.value = it }
}

class UserPreferencesDismissalsTest {
    @Test
    fun `forgetting a prefix drops only the dismissals that carry it`() = runTest {
        val repository =
            UserPreferencesRepository(
                InMemoryStore(
                    UserPreferences.newBuilder()
                        .addDismissedNotices("notice-1")
                        .addDismissedNotices("strike:1a2b")
                        .addDismissedNotices("strike:3c4d")
                        .build()
                ),
                BuildVersion("1.0", 1),
            )

        repository.forgetDismissedNotices("strike:")

        assertEquals(listOf("notice-1"), repository.dismissedNotices().first())
    }
}
