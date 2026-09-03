package com.warrenbrowse.vpn.feature.home.impl.connect

/**
 * One render effect per distinct radius, reused across frames.
 *
 * A `graphicsLayer` lambda runs on every frame, and a `BlurEffect` built
 * inside it is a fresh allocation per layer per frame: during the 6 s
 * connecting zoom the blur radius settles after 900 ms, so 5.1 s of frames
 * rebuilt an effect equal to the last one, twice per landscape. Keeping the
 * last effect and its radius turns those frames into a comparison.
 *
 * Draw lambdas run on the UI thread, so no synchronisation is needed.
 */
internal class RenderEffectCache<T : Any>(private val create: (radiusPx: Float) -> T) {
    private var radiusPx = Float.NaN
    private var effect: T? = null

    /**
     * The effect for [radiusPx], or `null` for a zero radius: a zero-radius
     * blur is invalid, and no blur means no effect.
     */
    fun effect(radiusPx: Float): T? {
        if (radiusPx <= 0f) return null
        if (radiusPx != this.radiusPx) {
            this.radiusPx = radiusPx
            effect = create(radiusPx)
        }
        return effect
    }
}
