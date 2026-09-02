package com.warrenbrowse.vpn.lib.repository

import com.warrenbrowse.vpn.lib.model.forum.ForumIdentity
import java.io.File

/** Where the problem happens; the forum form's first dropdown. */
enum class ReportArea(val token: String) {
    BROWSING("browsing"),
    CONNECTION("connection"),
    WALLET("wallet"),
    INSTALL("install"),
    OTHER("other"),
}

/** How often it happens; the forum form's last dropdown. */
enum class ReportFrequency(val token: String) {
    ALWAYS("always"),
    SOMETIMES("sometimes"),
    ONCE("once"),
}

/** The form a user fills in the app; the field names are the connect contract's. */
data class ReportForm(
    val area: ReportArea,
    val frequency: ReportFrequency,
    val whatHappened: String,
    val steps: String?,
)

/** A redacted problem report collected into a temporary file. */
data class CollectedReport(val file: File, val bytes: Long)

/** What the forum broker made of an in-app report. */
sealed interface ReportSubmitOutcome {
    /** The topic exists; [logs] is `attached`, `partial` or `none`. */
    data class Created(
        val topicId: Long,
        val topicUrl: String,
        val identity: ForumIdentity?,
        val logs: String,
    ) : ReportSubmitOutcome

    /** Never paid: the guest help form on the website is the channel. */
    data object SubscriptionRequired : ReportSubmitOutcome

    /** The device clock is off by more than the broker's window. */
    data object ClockSkew : ReportSubmitOutcome

    /** Over the per-wallet or global budget: wait. */
    data object RateLimited : ReportSubmitOutcome

    /** The logs are over a size cap: send without them. */
    data object TooLarge : ReportSubmitOutcome

    /** A field is outside its caps: fix the form. */
    data object Invalid : ReportSubmitOutcome

    /** The broker failed on its side: nothing the reporter can do now. */
    data object ServerError : ReportSubmitOutcome

    /** No wallet on device: nothing to sign with. */
    data object WalletNotReady : ReportSubmitOutcome

    /**
     * Not sent: the tunnel is between states ([ForumPreflight]), so the
     * connect host could not be resolved. Nothing was collected; the form is
     * intact and the user retries once the tunnel is connected or off.
     */
    data class Deferred(val tunnelClass: String) : ReportSubmitOutcome

    /** Anything else, with its class (`transport`, `build`, `http-502`, ...). */
    data class Failure(val reason: String) : ReportSubmitOutcome
}

/**
 * Collects the redacted problem report and files the in-app bug report. The
 * production impl lives in the app module (it needs the JNI bridge, the
 * wallet and the platform readers); the feature screens see only this seam.
 */
interface WarrenSupportReporter {
    /**
     * Whether a send may leave now, from the live tunnel state. Read before
     * collecting: a deferred send must cost nothing and change nothing.
     */
    fun preflight(): ForumPreflight

    /** Collects the redacted report into a temporary file the user may read. */
    suspend fun collect(): Result<CollectedReport>

    /** Signs and sends the report, with the collected logs when given. */
    suspend fun submit(form: ReportForm, report: CollectedReport?): ReportSubmitOutcome

    /** Deletes a collected report the user decided not to send. */
    fun discard(report: CollectedReport)
}

/**
 * Hands a sign-in code typed by the user to the same consent prompt a deep
 * link raises: the browser-independent path into the forum login.
 */
fun interface ForumSignInRequests {
    fun requestSignIn(sid: String)
}
