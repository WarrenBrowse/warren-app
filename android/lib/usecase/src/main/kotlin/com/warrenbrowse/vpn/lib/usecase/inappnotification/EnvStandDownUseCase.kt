package com.warrenbrowse.vpn.lib.usecase.inappnotification

import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.repository.WarrenEnvStandDown
import com.warrenbrowse.vpn.lib.repository.WarrenEnvStandDownStore
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map

/** A boolean the stand-down takes away and the manual re-enable puts back. */
class StandDownSetting(val read: () -> Boolean, val write: (Boolean) -> Unit)

/**
 * Coexistence with a higher-priority product environment: prod outranks
 * staging, staging outranks beta, and the outranked build is the one that
 * stands down. Nothing here ever commands the other install, and nothing in the
 * production build is modified, which is what keeps the design from becoming a
 * documented way for a local app to disarm someone's kill switch.
 *
 * The desktop daemon watches the higher environment's live state and yields
 * continuously. Mobile cannot: neither OS lets one app observe another app's
 * VPN state, and both already enforce that a single VPN is active at a time.
 * So the rule is the other install's PRESENCE, and it fires on the transition,
 * once, rather than on every evaluation. A continuous rule would undo the
 * user's manual re-enable at the next app start, since the other install is
 * still there.
 *
 * [refresh] is the whole observation: call it when the process starts. The
 * banner it raises is cleared by [reEnable] and by nothing else.
 */
class EnvStandDownUseCase(
    private val store: WarrenEnvStandDownStore,
    private val higherEnvironmentInstalled: () -> Boolean,
    private val stopTunnel: () -> Unit,
    private val autoConnect: StandDownSetting,
    private val blockingPolicy: StandDownSetting,
) : InAppNotificationUseCase {

    private val standingDown = MutableStateFlow(store.readEnvStandDown().standingDown())

    override operator fun invoke(): Flow<InAppNotification?> =
        standingDown
            .map { if (it) InAppNotification.EnvStandDown else null }
            .distinctUntilChanged()

    /**
     * Observe the device once. A presence that has not changed since the last
     * observation does nothing at all, so a re-enable holds and a build that
     * already stood down is not torn down again on every start.
     */
    fun refresh() {
        val record = store.readEnvStandDown()
        val installed = higherEnvironmentInstalled()
        if (installed == record.higherEnvironmentSeen) return
        if (installed) standDown() else forget(record)
    }

    /**
     * The user asked for this build back. Sticky by construction: the record
     * keeps the higher install marked as seen, so no later start stands down
     * for it again. Only a fresh install of it is a new transition.
     */
    fun reEnable() {
        val record = store.readEnvStandDown()
        if (!record.standingDown()) return
        store.writeEnvStandDown(record.copy(reEnabled = true))
        standingDown.value = false
        blockingPolicy.write(record.restoreBlockingPolicy)
        autoConnect.write(record.restoreAutoConnect)
    }

    /**
     * The order is the safety, and it is the desktop daemon's order
     * (`warren_env_arbitration::stand_down_plan`): record what is about to be
     * given up, take the tunnel down while the block is still armed, and lift
     * the block only then. Lifting it first would leave the device with no
     * tunnel and no block for the whole teardown. Auto-connect goes last so a
     * reboot cannot bring this build up underneath the one that holds the
     * device.
     */
    private fun standDown() {
        store.writeEnvStandDown(
            WarrenEnvStandDown(
                higherEnvironmentSeen = true,
                reEnabled = false,
                restoreAutoConnect = autoConnect.read(),
                restoreBlockingPolicy = blockingPolicy.read(),
            )
        )
        standingDown.value = true
        stopTunnel()
        blockingPolicy.write(false)
        autoConnect.write(false)
    }

    /**
     * The higher install is gone. What this build gave up for it goes back
     * before anything else, because the record about to be dropped is the only
     * place those two values are written down: dropping it first leaves the
     * user with the kill switch and the auto-connect off for good, the banner
     * gone, and nothing on screen that ever said they had been turned off.
     *
     * A record the user already re-enabled restored them then, so it is left
     * alone: writing it again would overwrite whatever they have chosen since.
     *
     * The record is then dropped whole rather than merely cleared of its
     * banner, so a reinstall later reads as the new transition it is and stands
     * this build down again.
     */
    private fun forget(record: WarrenEnvStandDown) {
        if (record.standingDown()) {
            blockingPolicy.write(record.restoreBlockingPolicy)
            autoConnect.write(record.restoreAutoConnect)
        }
        store.writeEnvStandDown(WarrenEnvStandDown())
        standingDown.value = false
    }
}

private fun WarrenEnvStandDown.standingDown(): Boolean = higherEnvironmentSeen && !reEnabled
