package com.warrenbrowse.vpn.lib.model.wallet

/**
 * Hex-encoded 32-byte Ed25519 public key derived from the wallet mnemonic.
 *
 * The hex form is what travels in the `X-Warren-Pubkey-Hex` request header
 * and what surfaces in any UI that needs a stable wallet identifier
 * (e.g. "your wallet" header in Settings). Construct via
 * `WarrenJni.mnemonicPubkeyHex(mnemonic)` (lowercase, no `0x` prefix).
 */
@JvmInline
value class WalletPubkeyHex(val value: String) {
    init {
        require(value.length == 64) { "wallet pubkey hex must be 64 chars, got ${value.length}" }
    }
}

/**
 * 12-word BIP39 mnemonic phrase, space-separated, lowercase. Held by the
 * wallet repository in encrypted storage; only ever passed through memory
 * just-in-time for a signing call.
 *
 * The class is intentionally NOT a `value class` over `String`: we want a
 * distinct type at every API boundary so accidental logs / serialisation
 * surface as type errors at compile time.
 */
class Mnemonic(val phrase: String) {
    init {
        val words = phrase.split(' ')
        require(words.size == 12 || words.size == 24) {
            "BIP39 mnemonic must be 12 or 24 words, got ${words.size}"
        }
    }

    override fun toString(): String =
        // Override `toString()` so accidental string-interpolation (e.g.
        // `"Mnemonic=$mnemonic"`) never leaks the phrase into a log.
        "Mnemonic(<redacted>)"
}
