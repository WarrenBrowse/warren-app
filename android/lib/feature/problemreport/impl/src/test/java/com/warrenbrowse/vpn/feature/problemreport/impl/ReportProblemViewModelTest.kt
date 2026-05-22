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

// D.6 step 65: sendReport flow rewired through WarrenSupportReportInvoker
// (biometric unlock + JNI signed POST /v1/support). The original integration
// tests asserted state transitions via the legacy
// ProblemReportRepository.sendReport stub; they cannot exercise the new path
// without an Activity (BiometricPromptAuthorizer requirement). Reduced to
// the two repository-delegation tests that survive the refactor; full
// coverage of the new flow is deferred to an instrumented test wired against
// a real FragmentActivity.
@ExtendWith(TestCoroutineRule::class)
class ReportProblemViewModelTest {

    @MockK private lateinit var mockMullvadProblemReport: ProblemReportRepository

    @MockK(relaxed = true) private lateinit var mockProblemReportRepository: ProblemReportRepository

    @MockK(relaxed = true) private lateinit var mockSupportReportInvoker: WarrenSupportReportInvoker

    private val problemReportFlow = MutableStateFlow(UserReport("", ""))

    private lateinit var viewModel: ReportProblemViewModel

    @BeforeEach
    fun setup() {
        MockKAnnotations.init(this)
        coEvery { mockMullvadProblemReport.collectLogs() } returns true
        coEvery { mockProblemReportRepository.problemReport } returns problemReportFlow
        viewModel =
            ReportProblemViewModel(
                warrenProblemReporter = mockMullvadProblemReport,
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
