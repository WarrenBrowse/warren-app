package com.warrenbrowse.vpn.lib.model

sealed interface CreateCustomListError

data object CustomListAlreadyExists : CreateCustomListError
