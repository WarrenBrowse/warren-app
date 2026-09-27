package com.warrenbrowse.vpn.app.service

import android.content.pm.PackageManager
import android.net.ConnectivityManager
import android.os.Build
import androidx.annotation.Keep
import androidx.annotation.RequiresApi
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.UnknownHostException

/**
 * The platform calls behind "Country per app" (docs/app-routing.md section 3.5): which uid owns
 * the socket of a flow seen on the TUN, and which packages that uid carries. The engine calls both
 * by name from `warren-jni/src/android_app_routes.rs`, once per new flow and once per uid, never
 * per packet; `proguard-rules.pro` keeps their names.
 *
 * Neither the addresses nor the answers are logged: which app talks to which host is the user's.
 */
@Keep
@RequiresApi(Build.VERSION_CODES.Q)
class FlowOwnerResolver(
    private val connectivityManager: ConnectivityManager,
    private val packageManager: PackageManager,
) {
    /**
     * The uid owning the socket of a TCP (`protocol` 6) or UDP (17) flow, or -1 when the platform
     * knows none. Only the active VPN app may ask, which Warren is while it carries the flow.
     */
    @Keep
    fun ownerUid(
        protocol: Int,
        local: ByteArray,
        localPort: Int,
        remote: ByteArray,
        remotePort: Int,
    ): Int =
        try {
            connectivityManager.getConnectionOwnerUid(
                protocol,
                InetSocketAddress(InetAddress.getByAddress(local), localPort),
                InetSocketAddress(InetAddress.getByAddress(remote), remotePort),
            )
        } catch (_: SecurityException) {
            // The VPN is no longer the active one: nothing to attribute.
            INVALID_UID
        } catch (_: IllegalArgumentException) {
            INVALID_UID
        } catch (_: UnknownHostException) {
            INVALID_UID
        }

    /** The packages of [uid], or null when it has none this app may see. */
    @Keep fun packagesForUid(uid: Int): Array<String>? = packageManager.getPackagesForUid(uid)

    private companion object {
        /** `Process.INVALID_UID`. */
        const val INVALID_UID = -1
    }
}
