package com.warrenbrowse.vpn.app.service

import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import java.util.concurrent.atomic.AtomicReference

/**
 * Single-process, one-shot mnemonic handoff channel from the
 * authenticating UI layer to [WarrenVpnService].
 *
 * Rationale: the mnemonic must not travel through Intent extras
 * (`Intent` parcels are visible to anyone with the right permission,
 * including system-level logging at sufficient verbosity). Since the
 * VPN service runs in the same Android process as the rest of the app
 * (`AndroidManifest.xml` does not declare `android:process=":vpn"`),
 * we can hand the secret over via a process-internal cache.
 *
 * Contract:
 *   1. UI calls [WalletRepository.unlock] -> obtains a [Mnemonic].
 *   2. UI calls [put] just before issuing the connect intent.
 *   3. Service calls [consume] inside `onStartCommand` to retrieve
 *      the mnemonic and clear the cache atomically. The cache is a
 *      one-shot: a second [consume] without a fresh [put] returns null.
 *   4. The reference is held inside an [AtomicReference] for thread-safe
 *      claim/release; the [Mnemonic.toString] override redacts the
 *      phrase from any logger or crash dump.
 *
 * The cache is intentionally not Koin-managed: it has no constructor
 * dependencies and a static identity is the correct model.
 */
object MnemonicCache {
    private val slot = AtomicReference<Mnemonic?>(null)

    /**
     * Stash a mnemonic for the next [consume]. Overwrites any prior
     * stash (last writer wins). Pass `null` to clear without consuming.
     */
    fun put(mnemonic: Mnemonic?) {
        slot.set(mnemonic)
    }

    /**
     * Atomically retrieve and clear the cached mnemonic. Returns `null`
     * if no mnemonic was staged. Callers must immediately use the
     * returned value (do not hold on to it beyond the connect handoff).
     */
    fun consume(): Mnemonic? = slot.getAndSet(null)

    /** True if a mnemonic is currently staged. Diagnostic use only. */
    fun isStaged(): Boolean = slot.get() != null
}
