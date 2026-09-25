package com.warrenbrowse.vpn.lib.repository

import com.warrenbrowse.vpn.lib.model.AccountStanding
import com.warrenbrowse.vpn.lib.model.StrikeNotice
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * The wallet's port-forward abuse standing as this installation last knew it
 * (warren-core doc 105 §5.4), `null` while nothing is known: no wallet, or an
 * API that does not report it yet.
 *
 * In memory only. The standing is re-read from the API on every resume, and
 * which strikes were already announced is Rust's to remember, in a ledger that
 * holds digests: a copy here on disk would be the one place the case
 * references outlive the account on the device.
 */
interface WarrenAccountStandingState {
    val standing: StateFlow<AccountStanding?>

    fun setStanding(standing: AccountStanding?)
}

class WarrenAccountStandingRepository : WarrenAccountStandingState {
    private val _standing = MutableStateFlow<AccountStanding?>(null)
    override val standing: StateFlow<AccountStanding?> = _standing.asStateFlow()

    override fun setStanding(standing: AccountStanding?) {
        _standing.value = standing
    }
}

/** The system notification a new strike raises. */
interface AccountStrikeAlerts {
    /** One notification for [notice]; each strike is handed over once. */
    fun announce(notice: StrikeNotice)

    /** The wallet left the device: its warnings come down from the shade. */
    fun clear()
}

/**
 * The native standing calls (`warren-jni` `standing` module). Kept apart from
 * [WarrenJniBridge] so a consumer and its fakes carry only these two.
 */
interface WarrenStandingBridge {
    /**
     * Polls the wallet's standing over a wallet-signed request and answers the
     * envelope `{"ok":..,"reported":..,"standing":..,"new_strikes":[..]}`.
     * Blocks on a network GET: invoke off the main thread.
     */
    fun accountStanding(mnemonic: String): String

    /** Forgets the standing of the wallet that left, on disk too. */
    fun forgetAccountStanding()
}
