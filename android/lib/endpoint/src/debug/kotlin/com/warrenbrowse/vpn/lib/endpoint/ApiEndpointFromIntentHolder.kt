package com.warrenbrowse.vpn.lib.endpoint

class ApiEndpointFromIntentHolder {
    var apiEndpointOverride: ApiEndpointOverride? = null
        private set

    fun setApiEndpointOverride(apiEndpointOverride: ApiEndpointOverride?) {
        this.apiEndpointOverride = apiEndpointOverride
    }
}
