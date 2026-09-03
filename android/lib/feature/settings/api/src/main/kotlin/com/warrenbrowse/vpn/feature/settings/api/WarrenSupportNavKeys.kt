package com.warrenbrowse.vpn.feature.settings.api

import com.warrenbrowse.vpn.core.NavKey2
import kotlinx.parcelize.Parcelize

/**
 * The in-app bug report: a form mirroring the forum's "Report a bug" template,
 * filed with the wallet signature and the redacted logs, for the user who
 * cannot complete the browser sign-in (warren-core doc 55).
 */
@Parcelize data object ReportProblemNavKey : NavKey2

/** The exact redacted report about to be sent, for the user to read first. */
@Parcelize data class ReportPreviewNavKey(val path: String) : NavKey2

/**
 * The forum sign-in code typed by hand: the browser-independent path into the
 * same consent prompt a `forum-login` deep link raises.
 */
@Parcelize data object ForumSignInCodeNavKey : NavKey2

/**
 * The community-forum activity panel (desktop `ForumActivityView`), opened
 * from the header bell and from the forum notification.
 */
@Parcelize data object ForumActivityNavKey : NavKey2
