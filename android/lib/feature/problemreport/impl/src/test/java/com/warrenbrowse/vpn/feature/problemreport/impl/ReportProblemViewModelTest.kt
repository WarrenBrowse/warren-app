package com.warrenbrowse.vpn.feature.problemreport.impl

import androidx.lifecycle.viewModelScope
import io.mockk.MockKAnnotations
import io.mockk.coEvery
import io.mockk.impl.annotations.MockK
import io.mockk.verify
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.UserReport
import com.warrenbrowse.vpn.lib.repository.ProblemReportRepository
import com.warrenbrowse.vpn.lib.repository.WarrenSupportReportInvoker
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith

// The sendReport flow goes through WarrenSupportReportInvoker (biometric
// unlock + JNI signed POST /v1/support), which needs an Activity
// (BiometricPromptAuthorizer requirement) and so cannot be exercised here.
// These tests cover repository delegation only; full coverage of the send
// flow belongs in an instrumented test wired against a real FragmentActivity.
@ExtendWith(TestCoroutineRule::class)
class ReportProblemViewModelTest {

    @MockK(relaxed = true) private lateinit var mockProblemReportRepository: ProblemReportRepository

    @MockK(relaxed = true) private lateinit var mockSupportReportInvoker: WarrenSupportReportInvoker

    private val problemReportFlow = MutableStateFlow(UserReport("", ""))

    private lateinit var viewModel: ReportProblemViewModel

    @BeforeEach
    fun setup() {
        MockKAnnotations.init(this)
        coEvery { mockProblemReportRepository.collectLogs() } returns true
        coEvery { mockProblemReportRepository.problemReport } returns problemReportFlow
        viewModel =
            ReportProblemViewModel(
                problemReportRepository = mockProblemReportRepository,
                isPlayBuild = false,
                supportReportInvoker = mockSupportReportInvoker,
            )
    }

    @AfterEach
    fun tearDown() {
        viewModel.viewModelScope.coroutineContext.cancel()
    }

    @Test
    fun `updateEmail should invoke setEmail on ProblemReportRepository`() = runTest {
        // Arrange
        val email = "my@email.com"

        // Act
        viewModel.updateEmail(email)

        // Assert
        verify { mockProblemReportRepository.setEmail(email) }
    }

    @Test
    fun `updateDescription should invoke updateDescription on ProblemReportRepository`() = runTest {
        // Arrange
        val description = "My description"

        // Act
        viewModel.updateDescription(description)

        // Assert
        verify { mockProblemReportRepository.setDescription(description) }
    }
}
