package com.warrenbrowse.vpn.app.perf

import android.util.Log

/** The logcat tag a tester filters on: `adb logcat -s WarrenJank`. */
const val JANK_LOG_TAG = "WarrenJank"

/**
 * Writes a [JankLogger] summary to logcat and nowhere else. The platform logger rather than the
 * app's: the app logger also writes its file, which a problem report carries, and frame timings
 * have no place there.
 */
fun logJankToLogcat(line: String) {
    Log.w(JANK_LOG_TAG, line)
}
