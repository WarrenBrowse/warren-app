package com.warrenbrowse.vpn.feature.home.impl.connect

import android.content.Context
import androidx.annotation.DrawableRes
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.res.imageResource
import java.util.concurrent.CountDownLatch

/**
 * Decoded scenery masters, one per drawable id, shared by every backdrop in
 * the process.
 *
 * `painterResource` decodes a bitmap drawable inside composition, on the main
 * thread, the first time each composable instance draws it: the first home
 * frame paid three 7.8 MB decodes, and every country change two more (the
 * incoming landscape, and the outgoing one again as the cross-fade's back
 * layer). Here a master is decoded once, and the splash and the backdrop warm
 * the ones the next frames will need on IO before any frame asks; a master
 * asked for cold is decoded on the spot, so an empty cache costs exactly what
 * `painterResource` cost and never more.
 *
 * Bounded to the masters one screen can show at once (the plain, the burrow,
 * Bula, the current landscape and the one it cross-fades from): the seven
 * masters together are 55 MB of ARGB, more than a 2 GB phone should keep for
 * art it is not drawing.
 */
internal class SceneryBitmapCache(
    private val decode: (Int) -> ImageBitmap,
    private val capacity: Int = DEFAULT_CAPACITY,
) {
    private val lock = Any()
    private val entries =
        object : LinkedHashMap<Int, ImageBitmap>(capacity, LOAD_FACTOR, true) {
            override fun removeEldestEntry(eldest: MutableMap.MutableEntry<Int, ImageBitmap>) =
                size > capacity
        }
    // A decode in flight per id, so a frame asking for a master the warm-up
    // is already decoding waits for that decode instead of running its own.
    private val inFlight = HashMap<Int, CountDownLatch>()

    /** The decoded master, decoded now when it is not cached yet. */
    fun get(@DrawableRes id: Int): ImageBitmap {
        val pending: CountDownLatch?
        val own: CountDownLatch?
        synchronized(lock) {
            entries[id]?.let {
                return it
            }
            pending = inFlight[id]
            own = if (pending == null) CountDownLatch(1).also { inFlight[id] = it } else null
        }
        // The decode itself runs outside the lock: a frame asking for another
        // master must not queue behind it.
        if (own != null) return decodeAndStore(id, own)
        pending?.await()
        // The other decode failed if this is still empty; try it here.
        synchronized(lock) { entries[id] }?.let {
            return it
        }
        return get(id)
    }

    private fun decodeAndStore(id: Int, latch: CountDownLatch): ImageBitmap {
        try {
            val decoded = decode(id)
            synchronized(lock) { entries[id] = decoded }
            return decoded
        } finally {
            synchronized(lock) { inFlight.remove(id) }
            latch.countDown()
        }
    }

    /** Decode [id] now if it is not cached, so the next [get] is a lookup. */
    fun warm(@DrawableRes id: Int) {
        get(id)
    }

    fun isWarm(@DrawableRes id: Int): Boolean = synchronized(lock) { entries.containsKey(id) }

    fun clear() = synchronized(lock) { entries.clear() }

    private companion object {
        const val DEFAULT_CAPACITY = 5
        const val LOAD_FACTOR = 0.75f
    }
}

/** The process-wide [SceneryBitmapCache], decoding from the app's resources. */
object SceneryBitmaps {
    @Volatile private var cache: SceneryBitmapCache? = null

    internal fun of(context: Context): SceneryBitmapCache =
        cache
            ?: synchronized(this) {
                cache
                    ?: run {
                        val resources = context.applicationContext.resources
                        SceneryBitmapCache(decode = { id -> ImageBitmap.imageResource(resources, id) })
                            .also { cache = it }
                    }
            }

    /**
     * Decode the masters the first home frame draws (the watched plain, the
     * burrow and Bula). Blocking: call it from IO, before the Connect screen is
     * reached, so that frame pays no decode on the main thread.
     */
    fun warmFirstFrame(context: Context) {
        val cache = of(context)
        firstFrameMasters().forEach(cache::warm)
    }

    /** Drop every decoded master; the next frame that needs one decodes it again. */
    fun clear() {
        cache?.clear()
    }
}
