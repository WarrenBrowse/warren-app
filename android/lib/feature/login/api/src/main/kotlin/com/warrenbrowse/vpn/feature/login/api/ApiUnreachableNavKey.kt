package com.warrenbrowse.vpn.feature.login.api

import android.os.Parcelable
import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.NavResult

@Parcelize data class ApiUnreachableNavKey(val action: LoginAction) : NavKey2

@Parcelize
enum class LoginAction : Parcelable {
    LOGIN,
    CREATE_ACCOUNT,
}

@Parcelize
sealed interface ApiUnreachableInfoDialogResult : NavResult {
    data class Success(val arg: ApiUnreachableNavKey) : ApiUnreachableInfoDialogResult

    data object Error : ApiUnreachableInfoDialogResult
}
