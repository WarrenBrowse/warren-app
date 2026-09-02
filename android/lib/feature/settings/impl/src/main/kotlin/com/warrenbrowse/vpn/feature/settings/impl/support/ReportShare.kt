package com.warrenbrowse.vpn.feature.settings.impl.support

import android.content.ClipData
import android.content.Context
import android.content.Intent
import androidx.core.content.FileProvider
import java.io.File

/** The provider serves the collected reports only (`res/xml/report_paths.xml`). */
private const val REPORT_PROVIDER_SUFFIX = ".reports"

/** The redacted report is plain text whatever its extension. */
internal const val REPORT_MIME_TYPE = "text/plain"

/**
 * The authority of the report `FileProvider`, derived from the package the
 * app actually runs as. Each product environment ships its own application id
 * (`com.warrenbrowse.vpn`, `.beta`, `.staging`), and the manifest declares the
 * provider with the `applicationId` placeholder, so a hardcoded prod authority
 * would resolve to nothing on the beta build the users have.
 */
internal fun reportProviderAuthority(packageName: String): String =
    packageName + REPORT_PROVIDER_SUFFIX

/** The name prefix of the copies handed to the share sheet. */
internal const val SHARED_REPORT_PREFIX = "shared-"

/**
 * The file the share sheet is actually handed: a copy of the report next to
 * it, named `shared-<report>`. The report itself belongs to the screen, which
 * deletes it when the logs are toggled off, when a send collects afresh or
 * when the screen goes away, while a receiver that resolves the content URI
 * lazily (a messenger, a file manager) may read it well after any of those.
 * The copy is nobody's to delete on the spot; the reporter prunes the report
 * directory of hour-old files at its next collection.
 */
internal fun sharedCopyOf(path: String): File {
    val report = File(path)
    val copy = File(report.parentFile, SHARED_REPORT_PREFIX + report.name)
    report.copyTo(copy, overwrite = true)
    return copy
}

/**
 * The share sheet for a collected report: the last-resort way for the logs to
 * leave the device when the connect broker itself is unreachable. The receiver
 * gets a read grant on this one file and nothing else; the file is the same
 * redacted report the preview shows.
 */
internal fun reportShareIntent(context: Context, path: String, chooserTitle: String): Intent {
    val uri =
        FileProvider.getUriForFile(
            context,
            reportProviderAuthority(context.packageName),
            sharedCopyOf(path),
        )
    val send =
        Intent(Intent.ACTION_SEND).apply {
            type = REPORT_MIME_TYPE
            putExtra(Intent.EXTRA_STREAM, uri)
            // The grant travels with the ClipData, which is what the system's
            // chooser reads for its preview; the extra alone leaves the
            // chooser without access (a permission denial in logcat, a blank
            // preview for the user).
            clipData = ClipData.newRawUri(null, uri)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
    return Intent.createChooser(send, chooserTitle).apply {
        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    }
}
