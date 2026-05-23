package com.warrenbrowse.vpn.lib.repository

import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import java.util.concurrent.atomic.AtomicReference

/**
 * Single-process, one-shot mnemonic handoff channel between callers
 * that hold a freshly-unlocked or freshly-generated [Mnemonic] and
 * the downstream consumer that needs to read it inside another
 * back-stack entry or process component.
 *
 * Two flows consume this:
 *   1. **Connect flow** (UI -> [WarrenVpnService]): the connect use
 *      case unlocks the wallet, stashes the mnemonic, dispatches the
 *      VPN-service-start intent, and the service consumes the slot
 *      inside `onStartCommand`. The intent itself carries no secret —
 *      Intent extras are visible to anyone with the right permission
 *      and can leak through verbose system logging.
 *   2. **Wallet onboarding** (login feature -> backup screen): the
 *      create-wallet flow stashes the freshly generated mnemonic and
 *      navigates to the backup screen with a sentinel NavKey. The
 *      backup screen consumes the slot at first composition. Audit
 *      follow-up: the previous design passed the phrase by value
 *      through the NavKey, which Compose Navigation persists in the
 *      saved-state Bundle — defeating the [Mnemonic] zero-on-close
 *      lifecycle.
 *
 * Both flows are sequential (the user cannot tap Connect while on
 * the backup screen), so a single slot is sufficient.
 *
 * Contract:
 *   - [put] stashes a mnemonic and atomically closes any previously
 *     staged one (zeroing its [CharArray] so the orphan does not
 *     linger on the heap).
 *   - [consume] atomically retrieves and clears the slot. Callers
 *     immediately use the returned value (typically inside a
 *     `mnemonic.use { ... }` block).
 *   - [isStaged] is diagnostic only.
 *
 * The cache is intentionally not Koin-managed: it has no constructor
 * dependencies and a static identity is the correct model. It lives
 * in `lib/repository` (not `:app`) so feature modules can consume
 * it without a forbidden upward dependency arrow.
 */
object MnemonicCache {
    private val slot = AtomicReference<Mnemonic?>(null)

    /**
     * Stash a mnemonic for the next [consume]. Overwrites any prior
     * stash (last writer wins). Pass `null` to clear without consuming.
     *
     * A previously-staged [Mnemonic] is [Mnemonic.close]d atomically
     * as part of the replace so its backing [CharArray] is zeroed
     * immediately - relying on GC would leave the secret resident on
     * the heap for an unbounded window after the orphan reference
     * becomes unreachable.
     */
    fun put(mnemonic: Mnemonic?) {
        val previous = slot.getAndSet(mnemonic)
        previous?.close()
    }

    /**
     * Atomically retrieve and clear the cached mnemonic. Returns `null`
     * if no mnemonic was staged. Callers must immediately use the
     * returned value (do not hold on to it beyond the handoff).
     */
    fun consume(): Mnemonic? = slot.getAndSet(null)

    /** True if a mnemonic is currently staged. Diagnostic use only. */
    fun isStaged(): Boolean = slot.get() != null
}
