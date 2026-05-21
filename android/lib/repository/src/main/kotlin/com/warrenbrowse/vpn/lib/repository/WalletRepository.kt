package com.warrenbrowse.vpn.lib.repository

import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.model.wallet.WalletPubkeyHex
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import kotlinx.coroutines.flow.StateFlow

/**
 * Wallet persistence contract.
 *
 * The single source of truth for the device's Warren wallet (mnemonic
 * encrypted by Android Keystore + EncryptedSharedPreferences). The
 * concrete `AndroidKeystoreWalletRepository` impl lives in
 * `lib/feature/wallet/impl/` (D.5).
 *
 * All methods that touch the mnemonic in cleartext are suspending so
 * callers must opt in to the I/O cost; the suspending boundary also
 * lets a future impl gate the decrypt step behind a `BiometricPrompt`
 * without changing the surface.
 */
interface WalletRepository {
    /** Observable wallet state. Emits a fresh value on every transition. */
    val state: StateFlow<WalletState>

    /**
     * Generate a fresh 12-word BIP39 mnemonic via `WarrenJni.generateMnemonic`,
     * persist it (Keystore-encrypted), and transition `state` to
     * [WalletState.Ready] with the derived pubkey. Returns the mnemonic
     * exactly once so the UI can prompt the user to back it up; subsequent
     * reads must go through [unlock].
     */
    suspend fun createWallet(): Mnemonic

    /**
     * Import an existing 12 / 24-word BIP39 mnemonic, persist it, and
     * transition `state` to [WalletState.Ready]. Returns the derived pubkey
     * for the caller to confirm against (e.g. server-side wallet binding).
     */
    suspend fun importWallet(mnemonic: Mnemonic): WalletPubkeyHex

    /**
     * Decrypt the persisted mnemonic just-in-time. Implementations gate
     * this behind a `BiometricPrompt` so the user explicitly authorises
     * each cleartext read. The returned `Mnemonic` reference must NOT be
     * stored; the caller passes it to the signing call and discards it.
     */
    suspend fun unlock(): Mnemonic

    /**
     * Erase the persisted wallet. Irreversible; the user is expected to
     * have backed up the mnemonic if they want to recover the same wallet
     * later via [importWallet].
     */
    suspend fun erase()
}
