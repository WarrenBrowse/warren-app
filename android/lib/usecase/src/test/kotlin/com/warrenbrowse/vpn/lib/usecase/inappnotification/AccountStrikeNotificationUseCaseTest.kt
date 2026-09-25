package com.warrenbrowse.vpn.lib.usecase.inappnotification

import app.cash.turbine.test
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.AbuseCategory
import com.warrenbrowse.vpn.lib.model.AccountStanding
import com.warrenbrowse.vpn.lib.model.AccountStrike
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.StrikeNotice
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.lib.repository.WarrenAccountStandingRepository
import io.mockk.every
import io.mockk.mockk
import kotlin.test.assertEquals
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertNull
import org.junit.jupiter.api.extension.ExtendWith

@ExtendWith(TestCoroutineRule::class)
class AccountStrikeNotificationUseCaseTest {

    private val state = WarrenAccountStandingRepository()
    private val dismissed = MutableStateFlow<List<String>>(emptyList())
    private val userPreferencesRepository: UserPreferencesRepository =
        mockk { every { dismissedNotices() } returns dismissed }
    private val useCase = AccountStrikeNotificationUseCase(state, userPreferencesRepository)

    private fun strike(reference: String) =
        AccountStrike(1_790_208_000, AbuseCategory.Copyright, "FI", 51413, reference)

    private fun standing(vararg references: String) =
        AccountStanding(references.map(::strike), threshold = 3, windowDays = 90, ban = null)

    @Test
    fun `no standing raises no banner`() = runTest { useCase().test { assertNull(awaitItem()) } }

    @Test
    fun `the banner warns about the newest strike with its rank`() = runTest {
        state.setStanding(standing("PF-1", "PF-2"))

        useCase().test {
            assertEquals(
                InAppNotification.AccountStrike(StrikeNotice(strike("PF-2"), 2, 3)),
                awaitItem(),
            )
        }
    }

    @Test
    fun `a dismissed strike stays put away and the next one raises the banner again`() = runTest {
        state.setStanding(standing("PF-1"))
        dismissed.value = listOf(strike("PF-1").dismissalKey)

        useCase().test {
            assertNull(awaitItem())
            state.setStanding(standing("PF-1", "PF-2"))
            assertEquals("PF-2", (awaitItem() as InAppNotification.AccountStrike).notice.strike.caseReference)
        }
    }
}
