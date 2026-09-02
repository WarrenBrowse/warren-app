package com.warrenbrowse.vpn.lib.model.forum

/**
 * The forum identity an approved sign-in (or an in-app report) hands back:
 * the pairwise handle the wallet posts under, which the app can learn nowhere
 * else (the derivation is keyed server side), and the position of this account
 * in the broadcast activity digest. Mirrors the desktop `ForumIdentity`.
 */
data class ForumIdentity(val handle: String, val notifySlot: Int?)
