package com.warrenbrowse.vpn.lib.repository

import android.content.Context
import android.content.SharedPreferences
import com.warrenbrowse.vpn.lib.model.forum.ForumIdentity
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Holds the forum identity learnt at the last approved sign-in or report, so
 * the account page can show the forum name and a later activity badge knows
 * its digest slot. The handle is public on the forum; what is private is its
 * link to this wallet, which is why it lives in app-private storage (never
 * backed up: `allowBackup=false`) and is erased with the wallet.
 */
interface ForumIdentityRepository {
    val identity: StateFlow<ForumIdentity?>

    fun save(identity: ForumIdentity)

    fun clear()
}

class SharedPreferencesForumIdentityRepository(context: Context) : ForumIdentityRepository {
    private val prefs: SharedPreferences =
        context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)

    private val _identity = MutableStateFlow(load())
    override val identity: StateFlow<ForumIdentity?> = _identity.asStateFlow()

    override fun save(identity: ForumIdentity) {
        val slot = identity.notifySlot
        prefs
            .edit()
            .putString(KEY_HANDLE, identity.handle)
            .apply { if (slot != null) putInt(KEY_SLOT, slot) else remove(KEY_SLOT) }
            .apply()
        _identity.value = identity
    }

    override fun clear() {
        prefs.edit().remove(KEY_HANDLE).remove(KEY_SLOT).apply()
        _identity.value = null
    }

    private fun load(): ForumIdentity? {
        val handle = prefs.getString(KEY_HANDLE, null) ?: return null
        val slot = if (prefs.contains(KEY_SLOT)) prefs.getInt(KEY_SLOT, -1) else null
        return ForumIdentity(handle, slot?.takeIf { it >= 0 })
    }

    private companion object {
        const val PREFS_NAME = "warren_forum"
        const val KEY_HANDLE = "handle"
        const val KEY_SLOT = "notify_slot"
    }
}
